// Eden ARK bridge — Phase 2 cutover на @kosmos/ark с kepler-aware resolution.
//
// Поведение:
//   1. При первом вызове `runArkRequest` (или явном `initArkRuntime()` из main.ts)
//      выполняется `ensureKeplerRunning(...)`.
//   2. Если Kepler host доступен (lock-file + PID alive + protocol_version matches) —
//      идём через WebSocket к нему (без spawn'а собственного ark-core-rpc).
//   3. Если Kepler недоступен И env `KOSMOS_KEPLER_OPTIONAL=1` — fallback на
//      self-managed sidecar (поведение pre-Phase-2).
//   4. Иначе — fail с понятным сообщением (Phase 6 final state).
//
// Public API `runArkRequest` / `shutdownArk` сохранена для обратной совместимости
// с `store.ts` (там 30+ call-sites через `runArkRequest({operation: ..., ...})`).
// Постепенная миграция на typed API @kosmos/ark (`client.objects.list()` и т.п.) —
// отдельная follow-up задача.

import fs from "node:fs";
import path from "node:path";

import electron from "electron";

import {
  ArkClient,
  ensureKeplerRunning,
  getArkDbPathForSelectedSpace,
  readSharedSelectedSpace,
} from "@kosmos/ark";

const { app } = electron;

interface ArkRequest {
  operation: string;
  [key: string]: unknown;
}

export interface ArkObjectRecord {
  id: string;
  typeId: string;
  title: string;
  contentJson: unknown;
  propsJson: Record<string, unknown>;
  createdAt: string;
  updatedAt: string;
  deletedAt?: string | null;
}

export interface ArkObjectTypeRecord {
  id: string;
  name: string;
  schemaJson: string;
  uiSchemaJson: string;
  createdAt: string;
  updatedAt: string;
  systemLocked: boolean;
}

export interface ArkObjectLinkRecord {
  id: string;
  sourceObjectId: string;
  targetObjectId: string;
  linkType: string;
  createdAt: string;
}

/**
 * Architectural model:
 *   Default — Kepler OPTIONAL. Без Kepler installed app работает standalone:
 *   spawn'ит свой ark-core-rpc child, читает/пишет общую `ark.db`, **без sync**.
 *   Если установлен Kepler host — app автоматически детектит lock-file и идёт
 *   через WS (получает sync + общий sidecar с другими апками).
 *
 *   `KOSMOS_REQUIRE_KEPLER=1` — для production / deployments, где запуск без
 *   Kepler нежелателен (например, чтобы гарантировать sync включён).
 *   `KOSMOS_KEPLER_OPTIONAL=1` — legacy флаг (no-op, fallback включён always).
 */
function isKeplerRequired(): boolean {
  return process.env.KOSMOS_REQUIRE_KEPLER === "1";
}

function isKeplerOptional(): boolean {
  // Backwards compat — flag всё ещё respected, но default behavior уже optional.
  return !isKeplerRequired();
}

function getArkBinaryPath(): string {
  const binaryName =
    process.platform === "win32" ? "ark-core-rpc.exe" : "ark-core-rpc";

  if (app.isPackaged) {
    return path.join(process.resourcesPath, "ark-core", binaryName);
  }

  const repoRoot = path.resolve(
    process.env.APP_ROOT ?? process.cwd(),
    "..",
    "..",
    "..",
  );
  const releasePath = path.join(
    repoRoot,
    "packages",
    "ark-core",
    "rust",
    "target",
    "release",
    binaryName,
  );
  if (fs.existsSync(releasePath)) {
    return releasePath;
  }

  return path.join(
    repoRoot,
    "packages",
    "ark-core",
    "rust",
    "target",
    "debug",
    binaryName,
  );
}

export function getArkDbPath(): string {
  const override = process.env.ARK_DB_PATH?.trim();
  if (override) {
    return path.resolve(override);
  }

  return getArkDbPathForSelectedSpace(
    app.getPath("appData"),
    readSharedSelectedSpace(app.getPath("appData")),
  );
}

function resolveSpaceId(): string {
  const selection = readSharedSelectedSpace(app.getPath("appData"));
  return selection?.spaceId ?? "eden-default";
}

function resolveDeviceId(): string {
  // Eden исторически не делал start_sync (sync принадлежит Kepler в Phase 5).
  // Stable deviceId на основе platform достаточен для HLC локального origin'а.
  return `eden-${process.platform}`;
}

let clientInstance: ArkClient | null = null;
let clientPromise: Promise<ArkClient> | null = null;

async function resolveClient(): Promise<ArkClient> {
  if (clientInstance) return clientInstance;
  if (clientPromise) return clientPromise;

  clientPromise = (async (): Promise<ArkClient> => {
    const state = await ensureKeplerRunning({
      appDataPath: app.getPath("appData"),
      waitMs: 10000,
      // Если Kepler required (флаг не set) — auto-launch. Если optional — пробуем
      // подключиться к запущенному, но не пытаемся стартовать сами (fallback path).
      autoLaunch: !isKeplerOptional(),
    });

    const spaceId = resolveSpaceId();
    const deviceId = resolveDeviceId();

    switch (state.kind) {
      case "connected": {
        console.log(
          `[eden.ark] using Kepler host (pid ${state.lock.pid}, ws_port ${state.lock.ws_port})`,
        );
        const client = new ArkClient({
          spaceId,
          deviceId,
          keplerLock: state.lock,
        });
        wireDriftEventLogging(client);
        clientInstance = client;
        return client;
      }

      case "incompatible-version": {
        throw new Error(
          `Kepler protocol mismatch: server ${state.keplerVersion.major}.${state.keplerVersion.minor}.${state.keplerVersion.patch}, ` +
            `client expects ${state.clientMajor}.x. Update Kepler or Eden.`,
        );
      }

      case "launch-failed":
      case "not-installed": {
        if (!isKeplerRequired()) {
          // Default path: standalone mode без sync. App работает локально.
          const detail =
            state.kind === "not-installed"
              ? "Kepler не установлен — standalone mode, sync disabled"
              : `Kepler недоступен (${state.reason}) — standalone fallback`;
          console.log(`[eden.ark] ${detail}`);
          const dbPath = getArkDbPath();
          fs.mkdirSync(path.dirname(dbPath), { recursive: true });
          const client = new ArkClient({
            spaceId,
            deviceId,
            dbPath,
            sidecarPath: getArkBinaryPath(),
          });
          wireDriftEventLogging(client);
          clientInstance = client;
          return client;
        }

        const detail =
          state.kind === "not-installed"
            ? `checked: ${state.checkedPaths.join(", ") || "(no candidates)"}`
            : state.reason;
        throw new Error(
          `Eden запущен с KOSMOS_REQUIRE_KEPLER=1, но Kepler ${state.kind} (${detail}). ` +
            `Установи Kosmos Kepler или сними флаг.`,
        );
      }
    }
  })();

  try {
    return await clientPromise;
  } catch (e) {
    // Reset чтобы можно было retry на следующем requeste.
    clientPromise = null;
    throw e;
  }
}

function wireDriftEventLogging(client: ArkClient): void {
  client.onArkEvent((event) => {
    if (event.event === "sync_error") {
      const code = String(event.code ?? "unknown");
      const entityId = String(event.entity_id ?? "");
      const awaitedTypeId = String(event.awaited_type_id ?? "");
      console.warn(
        `[eden.ark] sync_error code=${code} entity_id=${entityId} awaited_type_id=${awaitedTypeId}`,
      );
    } else if (event.event === "sync_replay") {
      const entityId = String(event.entity_id ?? "");
      const typeId = String(event.type_id ?? "");
      console.log(
        `[eden.ark] sync_replay entity_id=${entityId} type_id=${typeId}`,
      );
    }
  });
}

/**
 * Явная инициализация runtime — pre-resolves kepler/self-managed mode.
 * Вызывается из `main.ts` после `await initStore()` (см. `app.whenReady` hook),
 * чтобы при первом `runArkRequest` не было задержки на discovery + handshake.
 */
export async function initArkRuntime(): Promise<void> {
  await resolveClient();
}

export async function runArkRequest<T>(request: ArkRequest): Promise<T> {
  const client = await resolveClient();
  return client.invokeOperation<T>(request);
}

export function shutdownArk(): void {
  const client = clientInstance;
  clientInstance = null;
  clientPromise = null;
  if (client) {
    // ArkClient.stop() async — fire-and-forget (старый contract sync).
    client.stop().catch((e: unknown) => {
      const msg = e instanceof Error ? e.message : String(e);
      console.warn(`[eden.ark] stop error: ${msg}`);
    });
  }
}
