import fs from "node:fs";
import http from "node:http";
import path from "node:path";
import crypto from "node:crypto";
import { ipcMain, safeStorage, shell } from "electron";
import {
  expandRequestedRange,
  shouldRefreshCoverage,
} from "../src/services/google-calendar/cache";
import {
  addDays,
  startOfDay,
  toRange,
} from "../src/services/google-calendar/date";
import {
  canRestoreGoogleSession,
  isAccessTokenExpired,
} from "../src/services/google-calendar/session";
import type {
  GoogleCalendarAccount,
  GoogleCalendarCalendar,
  GoogleCalendarConfigState,
  GoogleCalendarEvent,
  GoogleCalendarRange,
  GoogleCalendarSnapshot,
} from "../src/services/google-calendar/contracts";

type SavedGoogleConfig = {
  clientId: string;
  clientSecret: string | null;
};

type SavedGoogleSession = {
  refreshToken: string | null;
  accessToken: string | null;
  accessTokenExpiresAt: string | null;
  scope: string | null;
  tokenType: string | null;
};

type SavedGoogleCache = {
  account: GoogleCalendarAccount | null;
  calendars: GoogleCalendarCalendar[];
  events: GoogleCalendarEvent[];
  lastSyncAt: string | null;
  lastSyncError: string | null;
  coverageStart: string | null;
  coverageEnd: string | null;
};

type GoogleTokenResponse = {
  access_token: string;
  expires_in: number;
  refresh_token?: string;
  scope?: string;
  token_type?: string;
  error?: string;
  error_description?: string;
};

type GoogleCalendarListResponse = {
  items?: Array<Record<string, unknown>>;
  nextPageToken?: string;
};

type GoogleEventsResponse = {
  items?: Array<Record<string, unknown>>;
  nextPageToken?: string;
};

type GoogleCalendarContext = {
  configPath: string;
  sessionPath: string;
  cachePath: string;
};

const GOOGLE_AUTH_SCOPES = [
  "openid",
  "email",
  "profile",
  "https://www.googleapis.com/auth/calendar.readonly",
].join(" ");

const GOOGLE_AUTH_URL = "https://accounts.google.com/o/oauth2/v2/auth";
const GOOGLE_TOKEN_URL = "https://oauth2.googleapis.com/token";
const GOOGLE_USERINFO_URL = "https://openidconnect.googleapis.com/v1/userinfo";
const GOOGLE_CALENDAR_LIST_URL =
  "https://www.googleapis.com/calendar/v3/users/me/calendarList";
const GOOGLE_EVENTS_BASE_URL =
  "https://www.googleapis.com/calendar/v3/calendars";
const CALLBACK_PATH = "/oauth2callback";

const EMPTY_CACHE: SavedGoogleCache = {
  account: null,
  calendars: [],
  events: [],
  lastSyncAt: null,
  lastSyncError: null,
  coverageStart: null,
  coverageEnd: null,
};

let syncInFlight: Promise<GoogleCalendarSnapshot> | null = null;
let syncBusy = false;
let queuedSyncRequest: {
  range?: GoogleCalendarRange;
  force: boolean;
} | null = null;
let queuedSyncPromise: Promise<GoogleCalendarSnapshot> | null = null;

function ensureDir(filePath: string) {
  fs.mkdirSync(path.dirname(filePath), { recursive: true });
}

function readProtectedJson<T>(filePath: string, fallback: T): T {
  try {
    if (!fs.existsSync(filePath)) return fallback;
    const raw = JSON.parse(fs.readFileSync(filePath, "utf-8")) as {
      encrypted?: boolean;
      payload?: string;
    };
    if (typeof raw.payload !== "string") return fallback;

    const decoded =
      raw.encrypted && safeStorage.isEncryptionAvailable()
        ? safeStorage.decryptString(Buffer.from(raw.payload, "base64"))
        : raw.payload;

    return JSON.parse(decoded) as T;
  } catch {
    return fallback;
  }
}

function writeProtectedJson<T>(filePath: string, value: T): void {
  ensureDir(filePath);

  const payload = JSON.stringify(value);
  const encrypted = safeStorage.isEncryptionAvailable();

  fs.writeFileSync(
    filePath,
    JSON.stringify(
      {
        encrypted,
        payload: encrypted
          ? safeStorage.encryptString(payload).toString("base64")
          : payload,
      },
      null,
      2,
    ),
    "utf-8",
  );
}

function deleteIfExists(filePath: string): void {
  try {
    fs.rmSync(filePath, { force: true });
  } catch {
    // ignore
  }
}

function readCache(context: GoogleCalendarContext): SavedGoogleCache {
  try {
    if (!fs.existsSync(context.cachePath)) return EMPTY_CACHE;
    const raw = JSON.parse(fs.readFileSync(context.cachePath, "utf-8"));
    return {
      ...EMPTY_CACHE,
      ...raw,
      calendars: Array.isArray(raw?.calendars) ? raw.calendars : [],
      events: Array.isArray(raw?.events) ? raw.events : [],
    } as SavedGoogleCache;
  } catch {
    return EMPTY_CACHE;
  }
}

function writeCache(context: GoogleCalendarContext, cache: SavedGoogleCache): void {
  ensureDir(context.cachePath);
  fs.writeFileSync(context.cachePath, JSON.stringify(cache, null, 2), "utf-8");
}

function readSavedConfig(context: GoogleCalendarContext): SavedGoogleConfig {
  return readProtectedJson<SavedGoogleConfig>(context.configPath, {
    clientId: "",
    clientSecret: null,
  });
}

function writeSavedConfig(
  context: GoogleCalendarContext,
  config: SavedGoogleConfig,
): void {
  if (!config.clientId.trim()) {
    deleteIfExists(context.configPath);
    return;
  }
  writeProtectedJson(context.configPath, config);
}

function readSavedSession(context: GoogleCalendarContext): SavedGoogleSession {
  return readProtectedJson<SavedGoogleSession>(context.sessionPath, {
    refreshToken: null,
    accessToken: null,
    accessTokenExpiresAt: null,
    scope: null,
    tokenType: null,
  });
}

function writeSavedSession(
  context: GoogleCalendarContext,
  session: SavedGoogleSession,
): void {
  if (!session.refreshToken && !session.accessToken) {
    deleteIfExists(context.sessionPath);
    return;
  }
  writeProtectedJson(context.sessionPath, session);
}

function getResolvedConfig(
  context: GoogleCalendarContext,
): SavedGoogleConfig & { state: GoogleCalendarConfigState } {
  const envClientId = process.env.GOOGLE_CALENDAR_CLIENT_ID?.trim() ?? "";
  const envClientSecret =
    process.env.GOOGLE_CALENDAR_CLIENT_SECRET?.trim() ?? "";
  const savedConfig = readSavedConfig(context);

  if (envClientId) {
    return {
      clientId: envClientId,
      clientSecret: envClientSecret || null,
      state: {
        clientId: envClientId,
        hasClientSecret: Boolean(envClientSecret),
        source: "env",
      },
    };
  }

  return {
    clientId: savedConfig.clientId.trim(),
    clientSecret: savedConfig.clientSecret?.trim() || null,
    state: {
      clientId: savedConfig.clientId.trim(),
      hasClientSecret: Boolean(savedConfig.clientSecret?.trim()),
      source: savedConfig.clientId ? "saved" : "none",
    },
  };
}

function buildSnapshot(context: GoogleCalendarContext): GoogleCalendarSnapshot {
  const cache = readCache(context);
  const config = getResolvedConfig(context);
  const session = readSavedSession(context);
  const connected = canRestoreGoogleSession({
    refreshToken: session.refreshToken,
    clientId: config.clientId,
  });

  return {
    config: config.state,
    account: cache.account,
    status: {
      available: true,
      configured: Boolean(config.clientId),
      connected,
      syncing: syncBusy,
      accountEmail: cache.account?.email ?? null,
      lastSyncAt: cache.lastSyncAt,
      lastSyncError: cache.lastSyncError,
      coverageStart: cache.coverageStart,
      coverageEnd: cache.coverageEnd,
    },
    calendars: cache.calendars,
    events: cache.events,
  };
}

function defaultRefreshRange(): GoogleCalendarRange {
  const today = startOfDay(new Date());
  return toRange(addDays(today, -30), addDays(today, 180));
}

function createPkceVerifier(): string {
  return crypto.randomBytes(48).toString("base64url");
}

function createPkceChallenge(verifier: string): string {
  return crypto
    .createHash("sha256")
    .update(verifier)
    .digest("base64url");
}

async function startInteractiveAuth(
  clientId: string,
  clientSecret: string | null,
): Promise<{
  accessToken: string;
  refreshToken: string | null;
  accessTokenExpiresAt: string;
  scope: string | null;
  tokenType: string | null;
}> {
  const verifier = createPkceVerifier();
  const challenge = createPkceChallenge(verifier);
  const state = crypto.randomBytes(16).toString("hex");

  const baseAuthUrl = new URL(GOOGLE_AUTH_URL);
  baseAuthUrl.searchParams.set("client_id", clientId);
  baseAuthUrl.searchParams.set("response_type", "code");
  baseAuthUrl.searchParams.set("scope", GOOGLE_AUTH_SCOPES);
  baseAuthUrl.searchParams.set("access_type", "offline");
  baseAuthUrl.searchParams.set("include_granted_scopes", "true");
  baseAuthUrl.searchParams.set("prompt", "consent select_account");
  baseAuthUrl.searchParams.set("state", state);
  baseAuthUrl.searchParams.set("code_challenge", challenge);
  baseAuthUrl.searchParams.set("code_challenge_method", "S256");

  const authResult = await new Promise<{
    code: string;
    redirectUri: string;
  }>((resolve, reject) => {
    let timeout: NodeJS.Timeout | null = null;
    const server = http.createServer((req, res) => {
      try {
        if (!req.url) throw new Error("Missing OAuth callback URL.");
        const callbackUrl = new URL(req.url, "http://127.0.0.1");
        if (callbackUrl.pathname !== CALLBACK_PATH) {
          res.writeHead(404);
          res.end("Not found");
          return;
        }

        const returnedState = callbackUrl.searchParams.get("state");
        const code = callbackUrl.searchParams.get("code");
        const error = callbackUrl.searchParams.get("error");

        if (returnedState !== state) throw new Error("OAuth state mismatch.");
        if (error) throw new Error(`Google OAuth error: ${error}`);
        if (!code) throw new Error("Google OAuth callback did not contain a code.");

        res.writeHead(200, { "Content-Type": "text/html; charset=utf-8" });
        res.end(
          "<html><body style=\"font-family: system-ui; padding: 24px;\">Вход завершён. Можно закрыть это окно и вернуться в Delphi.</body></html>",
        );

        if (timeout) clearTimeout(timeout);
        const redirectUri = `http://127.0.0.1:${(server.address() as http.AddressInfo).port}${CALLBACK_PATH}`;
        server.close();
        resolve({ code, redirectUri });
      } catch (error) {
        res.writeHead(400, { "Content-Type": "text/html; charset=utf-8" });
        res.end(
          "<html><body style=\"font-family: system-ui; padding: 24px;\">Не удалось завершить вход. Вернитесь в Delphi и попробуйте снова.</body></html>",
        );
        if (timeout) clearTimeout(timeout);
        server.close();
        reject(error);
      }
    });

    server.listen(0, "127.0.0.1", async () => {
      const address = server.address() as http.AddressInfo;
      const redirectUri = `http://127.0.0.1:${address.port}${CALLBACK_PATH}`;
      baseAuthUrl.searchParams.set("redirect_uri", redirectUri);

      timeout = setTimeout(() => {
        server.close();
        reject(new Error("OAuth timed out."));
      }, 2 * 60 * 1000);

      try {
        await shell.openExternal(baseAuthUrl.toString());
      } catch (error) {
        if (timeout) clearTimeout(timeout);
        server.close();
        reject(error);
      }
    });
  });

  const body = new URLSearchParams({
    code: authResult.code,
    client_id: clientId,
    code_verifier: verifier,
    grant_type: "authorization_code",
    redirect_uri: authResult.redirectUri,
  });

  if (clientSecret) {
    body.set("client_secret", clientSecret);
  }

  const response = await fetch(GOOGLE_TOKEN_URL, {
    method: "POST",
    headers: {
      "Content-Type": "application/x-www-form-urlencoded",
    },
    body,
  });

  const data = (await response.json()) as GoogleTokenResponse;

  if (!response.ok || !data.access_token) {
    throw new Error(data.error_description || data.error || "Google token exchange failed.");
  }

  return {
    accessToken: data.access_token,
    refreshToken: data.refresh_token ?? null,
    accessTokenExpiresAt: new Date(
      Date.now() + data.expires_in * 1000,
    ).toISOString(),
    scope: data.scope ?? null,
    tokenType: data.token_type ?? null,
  };
}

async function fetchJson<T>(
  url: string,
  accessToken: string,
): Promise<T> {
  const response = await fetch(url, {
    headers: {
      Authorization: `Bearer ${accessToken}`,
    },
  });

  if (!response.ok) {
    const message = await response.text();
    throw new Error(message || `Google request failed (${response.status})`);
  }

  return (await response.json()) as T;
}

async function refreshAccessToken(
  context: GoogleCalendarContext,
  config: SavedGoogleConfig,
  session: SavedGoogleSession,
): Promise<SavedGoogleSession> {
  if (!session.refreshToken) {
    throw new Error("Google refresh token is missing.");
  }

  const body = new URLSearchParams({
    client_id: config.clientId,
    grant_type: "refresh_token",
    refresh_token: session.refreshToken,
  });

  if (config.clientSecret) {
    body.set("client_secret", config.clientSecret);
  }

  const response = await fetch(GOOGLE_TOKEN_URL, {
    method: "POST",
    headers: {
      "Content-Type": "application/x-www-form-urlencoded",
    },
    body,
  });

  const data = (await response.json()) as GoogleTokenResponse;
  if (!response.ok || !data.access_token) {
    if (data.error === "invalid_grant") {
      writeSavedSession(context, {
        refreshToken: null,
        accessToken: null,
        accessTokenExpiresAt: null,
        scope: null,
        tokenType: null,
      });
    }
    throw new Error(data.error_description || data.error || "Google token refresh failed.");
  }

  const nextSession: SavedGoogleSession = {
    ...session,
    accessToken: data.access_token,
    accessTokenExpiresAt: new Date(
      Date.now() + data.expires_in * 1000,
    ).toISOString(),
    tokenType: data.token_type ?? session.tokenType,
    scope: data.scope ?? session.scope,
  };
  writeSavedSession(context, nextSession);
  return nextSession;
}

async function ensureAccessToken(
  context: GoogleCalendarContext,
  config: SavedGoogleConfig,
  session: SavedGoogleSession,
): Promise<SavedGoogleSession> {
  if (
    session.accessToken &&
    !isAccessTokenExpired(session.accessTokenExpiresAt)
  ) {
    return session;
  }
  return refreshAccessToken(context, config, session);
}

async function fetchGoogleProfile(accessToken: string): Promise<GoogleCalendarAccount> {
  const data = (await fetchJson<Record<string, unknown>>(
    GOOGLE_USERINFO_URL,
    accessToken,
  )) as Record<string, unknown>;

  return {
    email: String(data.email ?? ""),
    name: typeof data.name === "string" ? data.name : null,
    pictureUrl: typeof data.picture === "string" ? data.picture : null,
  };
}

function normalizeCalendar(item: Record<string, unknown>): GoogleCalendarCalendar {
  return {
    id: String(item.id ?? ""),
    summary: String(item.summary ?? item.id ?? "Без названия"),
    primary: Boolean(item.primary),
    selected: item.selected !== false,
    accessRole: typeof item.accessRole === "string" ? item.accessRole : null,
    backgroundColor:
      typeof item.backgroundColor === "string" ? item.backgroundColor : null,
    foregroundColor:
      typeof item.foregroundColor === "string" ? item.foregroundColor : null,
  };
}

function normalizeEvent(
  item: Record<string, unknown>,
  calendar: GoogleCalendarCalendar,
): GoogleCalendarEvent | null {
  const start = item.start as Record<string, unknown> | undefined;
  const end = item.end as Record<string, unknown> | undefined;
  const startValue =
    (typeof start?.dateTime === "string" ? start.dateTime : null) ??
    (typeof start?.date === "string" ? start.date : null);
  const endValue =
    (typeof end?.dateTime === "string" ? end.dateTime : null) ??
    (typeof end?.date === "string" ? end.date : null);

  if (!startValue || !endValue) return null;

  return {
    id: String(item.id ?? crypto.randomUUID()),
    calendarId: calendar.id,
    calendarSummary: calendar.summary,
    title: String(item.summary ?? "Без названия"),
    description:
      typeof item.description === "string" ? item.description : null,
    location: typeof item.location === "string" ? item.location : null,
    status: typeof item.status === "string" ? item.status : "confirmed",
    htmlLink: typeof item.htmlLink === "string" ? item.htmlLink : null,
    start: startValue,
    end: endValue,
    isAllDay:
      typeof start?.date === "string" && typeof end?.date === "string",
    color:
      typeof item.backgroundColor === "string"
        ? item.backgroundColor
        : calendar.backgroundColor,
  };
}

async function fetchAllCalendars(accessToken: string): Promise<GoogleCalendarCalendar[]> {
  const calendars: GoogleCalendarCalendar[] = [];
  let pageToken = "";

  do {
    const url = new URL(GOOGLE_CALENDAR_LIST_URL);
    if (pageToken) {
      url.searchParams.set("pageToken", pageToken);
    }

    const data = await fetchJson<GoogleCalendarListResponse>(
      url.toString(),
      accessToken,
    );
    for (const item of data.items ?? []) {
      calendars.push(normalizeCalendar(item));
    }
    pageToken = data.nextPageToken ?? "";
  } while (pageToken);

  return calendars.filter((calendar) => calendar.selected);
}

async function fetchCalendarEvents(
  accessToken: string,
  calendar: GoogleCalendarCalendar,
  range: GoogleCalendarRange,
): Promise<GoogleCalendarEvent[]> {
  const events: GoogleCalendarEvent[] = [];
  let pageToken = "";

  do {
    const url = new URL(
      `${GOOGLE_EVENTS_BASE_URL}/${encodeURIComponent(calendar.id)}/events`,
    );
    url.searchParams.set("singleEvents", "true");
    url.searchParams.set("orderBy", "startTime");
    url.searchParams.set("timeMin", range.start);
    url.searchParams.set("timeMax", range.end);
    url.searchParams.set("showDeleted", "false");
    url.searchParams.set("maxResults", "500");
    if (pageToken) {
      url.searchParams.set("pageToken", pageToken);
    }

    const data = await fetchJson<GoogleEventsResponse>(url.toString(), accessToken);
    for (const item of data.items ?? []) {
      const normalized = normalizeEvent(item, calendar);
      if (normalized) events.push(normalized);
    }
    pageToken = data.nextPageToken ?? "";
  } while (pageToken);

  return events;
}

async function syncGoogleCalendar(
  context: GoogleCalendarContext,
  requestedRange?: GoogleCalendarRange,
  force = false,
): Promise<GoogleCalendarSnapshot> {
  if (syncInFlight) {
    if (!requestedRange && !force) {
      return syncInFlight;
    }

    queuedSyncRequest = {
      range: requestedRange ?? queuedSyncRequest?.range,
      force: queuedSyncRequest?.force === true || force,
    };

    if (!queuedSyncPromise) {
      queuedSyncPromise = syncInFlight.then(() => {
        const nextRequest = queuedSyncRequest;
        queuedSyncRequest = null;
        queuedSyncPromise = null;

        if (!nextRequest) {
          return buildSnapshot(context);
        }

        return syncGoogleCalendar(context, nextRequest.range, nextRequest.force);
      });
    }

    return queuedSyncPromise;
  }

  const run = async () => {
    syncBusy = true;
    try {
      const resolvedConfig = getResolvedConfig(context);
      const session = readSavedSession(context);
      const cache = readCache(context);

      if (!canRestoreGoogleSession({
        refreshToken: session.refreshToken,
        clientId: resolvedConfig.clientId,
      })) {
        return buildSnapshot(context);
      }

      const range = requestedRange ?? defaultRefreshRange();
      const expandedRange = expandRequestedRange(range.start, range.end, 14);

      if (
        !force &&
        !shouldRefreshCoverage({
          coverageStart: cache.coverageStart,
          coverageEnd: cache.coverageEnd,
          lastSyncAt: cache.lastSyncAt,
          requestedStart: expandedRange.start,
          requestedEnd: expandedRange.end,
        })
      ) {
        return buildSnapshot(context);
      }

      const refreshedSession = await ensureAccessToken(
        context,
        resolvedConfig,
        session,
      );

      if (!refreshedSession.accessToken) {
        throw new Error("Google access token is missing after refresh.");
      }

      const [account, calendars] = await Promise.all([
        fetchGoogleProfile(refreshedSession.accessToken),
        fetchAllCalendars(refreshedSession.accessToken),
      ]);

      const eventLists = await Promise.all(
        calendars.map((calendar) =>
          fetchCalendarEvents(
            refreshedSession.accessToken!,
            calendar,
            expandedRange,
          ),
        ),
      );

      const nextCache: SavedGoogleCache = {
        account,
        calendars,
        events: eventLists.flat(),
        lastSyncAt: new Date().toISOString(),
        lastSyncError: null,
        coverageStart: expandedRange.start,
        coverageEnd: expandedRange.end,
      };
      writeCache(context, nextCache);
      return buildSnapshot(context);
    } catch (error) {
      const cache = readCache(context);
      writeCache(context, {
        ...cache,
        lastSyncError:
          error instanceof Error ? error.message : "Google sync failed.",
      });
      return buildSnapshot(context);
    } finally {
      syncBusy = false;
      syncInFlight = null;
    }
  };

  syncInFlight = run();
  return syncInFlight;
}

export function registerGoogleCalendarIpc(getDataDir: () => string): void {
  const context: GoogleCalendarContext = {
    configPath: path.join(getDataDir(), "google-calendar", "oauth-config.json"),
    sessionPath: path.join(getDataDir(), "google-calendar", "session.json"),
    cachePath: path.join(getDataDir(), "google-calendar", "cache.json"),
  };

  ipcMain.handle("google-calendar:getSnapshot", async () => buildSnapshot(context));

  ipcMain.handle(
    "google-calendar:setConfig",
    async (_event, payload: { clientId?: string; clientSecret?: string }) => {
      const current = readSavedConfig(context);
      const clientId = payload.clientId?.trim() ?? "";
      const nextConfig: SavedGoogleConfig = {
        clientId,
        clientSecret:
          payload.clientSecret?.trim() ||
          (clientId && current.clientId === clientId ? current.clientSecret : null),
      };

      writeSavedConfig(context, nextConfig);
      return buildSnapshot(context);
    },
  );

  ipcMain.handle("google-calendar:signOut", async () => {
    writeSavedSession(context, {
      refreshToken: null,
      accessToken: null,
      accessTokenExpiresAt: null,
      scope: null,
      tokenType: null,
    });
    writeCache(context, EMPTY_CACHE);
    return buildSnapshot(context);
  });

  ipcMain.handle("google-calendar:signIn", async () => {
    const resolvedConfig = getResolvedConfig(context);
    if (!resolvedConfig.clientId) {
      throw new Error("Google OAuth Client ID is not configured.");
    }

    syncBusy = true;
    try {
      const auth = await startInteractiveAuth(
        resolvedConfig.clientId,
        resolvedConfig.clientSecret,
      );
      writeSavedSession(context, {
        refreshToken: auth.refreshToken,
        accessToken: auth.accessToken,
        accessTokenExpiresAt: auth.accessTokenExpiresAt,
        scope: auth.scope,
        tokenType: auth.tokenType,
      });
    } finally {
      syncBusy = false;
    }

    return syncGoogleCalendar(context, defaultRefreshRange(), true);
  });

  ipcMain.handle(
    "google-calendar:refresh",
    async (_event, range?: GoogleCalendarRange, force?: boolean) =>
      syncGoogleCalendar(context, range, force ?? false),
  );
}
