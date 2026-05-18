// Marketplace integration: fetch catalog.json от kosmos-extensions + download
// .kext по URL и delegate в существующий installFromPath flow.
//
// Catalog source: https://raw.githubusercontent.com/yoso-industries/kosmos-extensions/main/catalog.json
//
// Cache: in-memory, 1h TTL. Force refresh — параметр `force` в catalogFetch.

import { ipcMain } from "electron";
import { createWriteStream, existsSync, mkdirSync, readFileSync, rmSync } from "node:fs";
import { tmpdir } from "node:os";
import path from "node:path";
import https from "node:https";
import crypto from "node:crypto";
import { installFromPath } from "./extension-installer";
import type { KextManifestPreview } from "./extension-installer";

export interface CatalogExtension {
  id: string;
  name: string;
  description: string;
  author: string;
  version: string;
  keplerApiVersion: string;
  iconUrl: string | null;
  downloadUrl: string;
  sha256: string | null;
  size: number | null;
}

export interface Catalog {
  schemaVersion: number;
  updatedAt: string;
  extensions: CatalogExtension[];
}

const CATALOG_URL =
  "https://raw.githubusercontent.com/yoso-industries/kosmos-extensions/main/catalog.json";
const CACHE_TTL_MS = 60 * 60 * 1000; // 1h

let cachedCatalog: { data: Catalog; fetchedAt: number } | null = null;

function httpsGetText(url: string, timeoutMs = 15000): Promise<string> {
  return new Promise((resolve, reject) => {
    const req = https.get(url, { headers: { "User-Agent": "Kepler-Shell" } }, (res) => {
      if (
        res.statusCode &&
        res.statusCode >= 300 &&
        res.statusCode < 400 &&
        res.headers.location
      ) {
        // Follow redirect (raw.githubusercontent.com обычно serves direct, но safety).
        httpsGetText(res.headers.location, timeoutMs).then(resolve, reject);
        res.resume();
        return;
      }
      if (!res.statusCode || res.statusCode >= 400) {
        reject(new Error(`HTTP ${res.statusCode} fetching ${url}`));
        res.resume();
        return;
      }
      const chunks: Buffer[] = [];
      res.on("data", (c) => chunks.push(Buffer.from(c)));
      res.on("end", () => resolve(Buffer.concat(chunks).toString("utf8")));
      res.on("error", reject);
    });
    req.on("error", reject);
    req.setTimeout(timeoutMs, () => {
      req.destroy(new Error(`timeout fetching ${url}`));
    });
  });
}

function httpsDownload(url: string, destPath: string, timeoutMs = 60000): Promise<void> {
  return new Promise((resolve, reject) => {
    const fetchUrl = (u: string, depth = 0) => {
      if (depth > 5) {
        reject(new Error(`too many redirects downloading ${url}`));
        return;
      }
      if (!u.startsWith("https://")) {
        reject(new Error(`redirect to non-HTTPS URL blocked: ${u}`));
        return;
      }
      const req = https.get(u, { headers: { "User-Agent": "Kepler-Shell" } }, (res) => {
        if (
          res.statusCode &&
          res.statusCode >= 300 &&
          res.statusCode < 400 &&
          res.headers.location
        ) {
          res.resume();
          fetchUrl(res.headers.location, depth + 1);
          return;
        }
        if (!res.statusCode || res.statusCode >= 400) {
          reject(new Error(`HTTP ${res.statusCode} downloading ${u}`));
          res.resume();
          return;
        }
        const file = createWriteStream(destPath);
        res.pipe(file);
        file.on("finish", () => file.close(() => resolve()));
        file.on("error", (e) => {
          rmSync(destPath, { force: true });
          reject(e);
        });
      });
      req.on("error", reject);
      req.setTimeout(timeoutMs, () => req.destroy(new Error(`timeout downloading ${u}`)));
    };
    fetchUrl(url);
  });
}

function sha256File(p: string): string {
  const h = crypto.createHash("sha256");
  h.update(readFileSync(p));
  return h.digest("hex");
}

export async function fetchCatalog(force = false): Promise<Catalog> {
  const now = Date.now();
  if (!force && cachedCatalog && now - cachedCatalog.fetchedAt < CACHE_TTL_MS) {
    return cachedCatalog.data;
  }
  const raw = await httpsGetText(CATALOG_URL);
  const parsed = JSON.parse(raw) as Catalog;
  if (!parsed || typeof parsed.schemaVersion !== "number") {
    throw new Error("catalog.json: invalid format (no schemaVersion)");
  }
  // Tolerate unknown fields. Reject incompatible major schema version.
  if (parsed.schemaVersion !== 1) {
    throw new Error(
      `catalog.json: schemaVersion ${parsed.schemaVersion} не поддерживается этой версией Kepler`,
    );
  }
  if (!Array.isArray(parsed.extensions)) {
    throw new Error("catalog.json: extensions must be an array");
  }
  cachedCatalog = { data: parsed, fetchedAt: now };
  return parsed;
}

export async function installFromUrl(
  url: string,
  expectedSha256?: string | null,
): Promise<KextManifestPreview> {
  if (typeof url !== "string" || !url.startsWith("https://")) {
    throw new Error("installFromUrl: только https:// URL");
  }
  const tmpRoot = path.join(tmpdir(), "kepler-ext-download");
  mkdirSync(tmpRoot, { recursive: true });
  const stamp = Date.now().toString(36) + Math.random().toString(36).slice(2, 6);
  const tmpFile = path.join(tmpRoot, `download-${stamp}.kext`);

  try {
    await httpsDownload(url, tmpFile);
    if (!existsSync(tmpFile)) {
      throw new Error(`download finished but file отсутствует: ${tmpFile}`);
    }
    if (expectedSha256 && expectedSha256.length > 0) {
      const actual = sha256File(tmpFile);
      if (actual.toLowerCase() !== expectedSha256.toLowerCase()) {
        throw new Error(
          `sha256 mismatch: expected ${expectedSha256}, got ${actual}`,
        );
      }
    }
    return installFromPath(tmpFile);
  } finally {
    try {
      rmSync(tmpFile, { force: true });
    } catch {
      /* ignore */
    }
  }
}

let registered = false;
let periodicTimer: NodeJS.Timeout | null = null;

/**
 * Стартует фоновый перефетч catalog.json каждые 24h. Cache TTL fetchCatalog —
 * 1h, поэтому если юзер открывает Settings часто, периодический check не
 * мешает. Цель: чтобы Settings → Маркетплейс badge сразу был актуален даже
 * на свежем старте launcher'а.
 */
export function startPeriodicCatalogCheck(): void {
  if (periodicTimer) return;
  // initial fetch на старте (не блокирующий)
  fetchCatalog(false).catch((e) =>
    console.error("[marketplace] initial catalog fetch failed:", e),
  );
  periodicTimer = setInterval(
    () => {
      fetchCatalog(true).catch((e) =>
        console.error("[marketplace] periodic catalog fetch failed:", e),
      );
    },
    24 * 60 * 60 * 1000,
  );
}

export function registerMarketplaceIpc(): void {
  if (registered) return;
  registered = true;

  ipcMain.handle(
    "kepler:extension:catalog:fetch",
    async (_e, force?: boolean): Promise<Catalog> => {
      return fetchCatalog(!!force);
    },
  );

  ipcMain.handle(
    "kepler:extension:install:fromUrl",
    async (
      _e,
      url: string,
      expectedSha256?: string | null,
    ): Promise<KextManifestPreview> => {
      return installFromUrl(url, expectedSha256 ?? null);
    },
  );
}
