import { existsSync, readFileSync, readdirSync } from "node:fs";
import path from "node:path";
import net from "node:net";
import { fileURLToPath } from "node:url";
import { KEPLER_API_VERSION, satisfiesSemver } from "./kepler-api";
import { loadCommandPackageManifest } from "./command-host/manifest";
import { resolveExtensionDir, resolveExtensionRoots } from "./extension-package-registry";
import type { ExtensionKind, ExtensionManifest } from "./extension-manifest-types";

export type { ExtensionManifest } from "./extension-manifest-types";

export {
  assertSafeUserDataName,
  ensureUserDataDir,
  extensionUserDataDir,
  resolveExtensionDir,
  resolveExtensionLocation,
  resolveSafeUserDataPath,
} from "./extension-package-registry";

const __filename = fileURLToPath(import.meta.url);
const __dirname = path.dirname(__filename);

// ---------------------------------------------------------------------------
// Developer mode
// ---------------------------------------------------------------------------

// Note: `isDeveloperModeActive()` удалён 2026-05-19. Source resolution
// extension'ов (Vite vs dist) теперь через TCP probe в `resolveExtensionSource()`,
// см. ниже. Settings toggle `developerMode` остаётся (legacy UI hint), но
// extension loader его больше не читает. См. `docs-site/concepts/extension-dev-mode.md`.

// ---------------------------------------------------------------------------
// Extension source resolution (Vite dev server vs dist bundle).
//
// Дизайн: probe-based auto-detect. Никаких persistent toggle'ов — runtime
// реальность важнее настройки. Если Vite dev server для extension'а
// поднят на `localhost:<manifest.devPort>` — грузим оттуда (HMR). Если порт
// мёртв — fallback на `dist/index.html`. В production (нет `VITE_DEV_SERVER_URL`)
// probe не запускается совсем — extension всегда из `dist/`.
//
// Это устраняет double opt-in (раньше нужно было `KEPLER_DEV_EXTENSIONS=1`
// env + `developerMode: true` в settings + руками поднять `dev:extensions`).
// Теперь `bun run dev` поднимает Vite-серверы автоматически (см. platform/desktop/scripts/dev.mjs),
// а extension-host сам выясняет к кому подключаться.
// ---------------------------------------------------------------------------

const PROBE_TIMEOUT_MS = 500;
const PROBE_ALIVE_CACHE_TTL_MS = 10_000;
const lastAliveAt = new Map<number, number>();

async function probeExtensionDevServer(port: number): Promise<boolean> {
  const cached = lastAliveAt.get(port);
  if (cached !== undefined && Date.now() - cached < PROBE_ALIVE_CACHE_TTL_MS) {
    return true;
  }
  const alive = await new Promise<boolean>((resolve) => {
    let resolved = false;
    const finish = (ok: boolean) => {
      if (resolved) return;
      resolved = true;
      try {
        sock.destroy();
      } catch {
        /* already destroyed */
      }
      resolve(ok);
    };
    const sock = net.createConnection({ host: "127.0.0.1", port });
    sock.once("connect", () => finish(true));
    sock.once("error", () => finish(false));
    sock.setTimeout(PROBE_TIMEOUT_MS, () => finish(false));
  });
  if (alive) lastAliveAt.set(port, Date.now());
  return alive;
}

export function isShellInDevSession(): boolean {
  // VITE_DEV_SERVER_URL выставляется vite-plugin-electron только в dev session.
  // В packaged production его нет → probe не делаем, всегда dist.
  return !!process.env.VITE_DEV_SERVER_URL;
}

function envFlag(name: string): boolean {
  return process.env[name] === "1";
}

export function isHeadless(): boolean {
  return envFlag("KOSMOS_HEADLESS");
}

export function isHeadlessOrTest(): boolean {
  return isHeadless() || envFlag("KOSMOS_TEST_MODE");
}

export interface ExtensionSource {
  kind: "dev-server" | "dist";
  url?: string;
  file?: string;
}

export async function resolveExtensionSource(
  id: string,
  manifest: ExtensionManifest,
  extensionDir: string,
): Promise<ExtensionSource | null> {
  if (manifest.kind === "native") {
    return null;
  }
  if (isShellInDevSession() && manifest.devPort) {
    const alive = await probeExtensionDevServer(manifest.devPort);
    if (alive) {
      return {
        kind: "dev-server",
        url: `http://localhost:${manifest.devPort}/`,
      };
    }
    console.log(
      `[kepler-shell] extension '${id}' dev server :${manifest.devPort} не отвечает — fallback на dist`,
    );
  }
  const entryHtml = resolveEntryHtml(manifest, extensionDir);
  if (!existsSync(entryHtml)) {
    console.error(
      `[kepler-shell] extension '${id}' entryHtml not found: ${entryHtml}` +
        ` — для Vue extension'а сначала запусти build (bun run build:extensions).`,
    );
    return null;
  }
  return { kind: "dist", file: entryHtml };
}

function resolveSharedPreloadPath(): string {
  // Bundled by vite-plugin-electron alongside main.js: dist-electron/extension-preload.mjs
  return path.join(__dirname, "extension-preload.mjs");
}

export function loadExtensionManifest(id: string): ExtensionManifest | null {
  const dir = resolveExtensionDir(id);
  if (!dir) return null;
  const manifestPath = path.join(dir, "manifest.json");
  if (!existsSync(manifestPath)) {
    const commandPackage = loadCommandPackageManifest(dir);
    if (!commandPackage) return null;
    if (commandPackage.name !== id) {
      console.warn(
        `[kepler-shell] Command package name mismatch: folder=${id}, package=${commandPackage.name}`,
      );
      return null;
    }
    return {
      id: commandPackage.name,
      name: commandPackage.title,
      version: commandPackage.version,
      description: commandPackage.description,
      author: commandPackage.author,
      permissions: commandPackage.kosmos?.permissions,
      kind: "command-extension",
      icon: commandPackage.icon,
      windowEffect: commandPackage.kosmos?.windowEffect,
      keplerApiVersion: commandPackage.kosmos?.minKosmosApiVersion,
      commandPackage,
      commands: commandPackage.commands.map((command) => ({
        id: command.name,
        title: command.title,
        subtitle: command.subtitle ?? commandPackage.title,
        icon: command.icon ?? commandPackage.icon,
        kind: "command",
        mode:
          command.mode === "no-view"
            ? "command-no-view"
            : command.mode === "menu-bar"
              ? "command-menu-bar"
              : "command-view",
      })),
    };
  }
  try {
    // SAFETY: The surrounding boundary establishes this documented contract.
    const manifest = JSON.parse(readFileSync(manifestPath, "utf8")) as ExtensionManifest;
    if (manifest.id !== id) {
      console.warn(
        `[kepler-shell] extension manifest id mismatch: folder=${id}, manifest=${String(
          manifest.id,
        )}`,
      );
      return null;
    }
    return manifest;
  } catch (e) {
    console.error(`[kepler-shell] extension manifest invalid: ${id}`, e);
    return null;
  }
}

export function listExtensions(): ExtensionManifest[] {
  // Collect ids из всех roots; dedup по id, выигрывает первый встреченный
  // (priority order — см. resolveExtensionRoots).
  const seen = new Set<string>();
  const out: ExtensionManifest[] = [];
  for (const root of resolveExtensionRoots()) {
    if (!existsSync(root)) continue;
    try {
      const entries = readdirSync(root, { withFileTypes: true });
      for (const entry of entries) {
        if (!entry.isDirectory()) continue;
        if (seen.has(entry.name)) continue;
        const m = loadExtensionManifest(entry.name);
        if (m) {
          seen.add(entry.name);
          out.push(m);
        }
      }
    } catch {
      /* ignore unreadable root */
    }
  }
  return out;
}

export function resolvePreloadForManifest(
  manifest: ExtensionManifest,
  extensionDir: string,
): string | undefined {
  const kind: ExtensionKind = manifest.kind ?? "static";
  // Shared preload по умолчанию для Vue extension'ов.
  if (!manifest.preload) {
    return kind === "vue" ? resolveSharedPreloadPath() : undefined;
  }
  // Спец-значение → shared preload.
  if (manifest.preload === "kepler-extension-preload.mjs") {
    return resolveSharedPreloadPath();
  }
  // Иначе — относительный путь внутри extension dir (legacy / custom).
  return path.join(extensionDir, manifest.preload);
}

function resolveEntryHtml(manifest: ExtensionManifest, extensionDir: string): string {
  if (!manifest.entryHtml) {
    throw new Error(`[kepler-shell] extension '${manifest.id}' has no entryHtml`);
  }
  // entryHtml интерпретируется относительно extension dir. Для vue это обычно
  // "dist/index.html" (после vite build), для static — "index.html".
  return path.join(extensionDir, manifest.entryHtml);
}

/**
 * Проверяет совместимость extension'а с текущей Kepler API версией. Если
 * `manifest.keplerApiVersion` указан и НЕ удовлетворяет current API version —
 * возвращает строку-причину; иначе null (всё ОК).
 *
 * Если поле отсутствует — считается legacy: вернёт null + warn в console.
 */
export function checkApiCompat(manifest: ExtensionManifest): string | null {
  if (!manifest.keplerApiVersion) {
    console.warn(
      `[kepler-shell] extension '${manifest.id}' has no keplerApiVersion — ` +
        `loading anyway (legacy). Add "keplerApiVersion": "^${KEPLER_API_VERSION}" в manifest.`,
    );
    return null;
  }
  if (satisfiesSemver(KEPLER_API_VERSION, manifest.keplerApiVersion)) {
    return null;
  }
  return (
    `Расширение «${manifest.name}» несовместимо с этим Kepler. ` +
    `Требуется Kepler API ${manifest.keplerApiVersion}, установлено ${KEPLER_API_VERSION}. ` +
    `Обновите расширение (новый .kext).`
  );
}
