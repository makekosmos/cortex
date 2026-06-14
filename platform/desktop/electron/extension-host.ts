// Extension host — управляет lifecycle Vue/static extension'ов внутри Kepler.
//
// Phase 4 contract для extension'ов:
//
//   extensions/<id>/
//     manifest.json         { id, name, kind: "vue" | "static",
//                             entryHtml: "dist/index.html" | "index.html",
//                             preload?, width, height, minWidth?, minHeight? }
//     src/                  (только для kind: "vue")
//       main.ts             // createApp(App).mount("#app")
//       App.vue
//       ...
//     index.html            (для "vue" — точка входа vite, ссылается на /src/main.ts;
//                            для "static" — финальный готовый html)
//     dist/                 (для "vue" — build output, entryHtml: "dist/index.html")
//
// Vue extension'ы получают `window.kepler` namespace через shared preload
// (extension-preload.mjs). API exposes:
//   - kepler.ark.request(operation, params)   — RPC к ARK через main proxy
//   - kepler.ark.subscribe(event, handler)    — события (commands_changed и т.п.)
//   - kepler.window.{close,minimize,maximize} — управление окном
//   - kepler.meta.id()                        — id текущего extension'а
//
// kind: "static" (legacy PoC) — preload не используется по умолчанию;
// extension сам отвечает за всю свою логику.

import {
  app,
  BrowserWindow,
  dialog,
  ipcMain,
  screen,
  type OpenDialogOptions,
  type SaveDialogOptions,
  type WebContents,
} from "electron";
import { spawn, type ChildProcess } from "node:child_process";
import {
  copyFileSync,
  existsSync,
  mkdirSync,
  readFileSync,
  readdirSync,
  rmSync,
  statSync,
  writeFileSync,
} from "node:fs";
import path from "node:path";
import net from "node:net";
import { fileURLToPath } from "node:url";
import { parseLocalImageRequestUrl } from "./local-image-protocol";
import { localImageUrl } from "./local-image-protocol";
import { keplerDataDir } from "./data-dir";
import { macWindowChrome } from "./mac-window";
import {
  applyWindowMaterial,
  backgroundMaterialOption,
  resolveWindowMaterial,
  type KosmosWindowMaterial,
} from "./window-effects";
import { KEPLER_API_VERSION, satisfiesSemver } from "./kepler-api";
import {
  installFromPath,
  listInstalledUserExtensions,
  previewSource,
  revertExtension,
  listBackups,
  uninstallExtension,
} from "./extension-installer";
import {
  assertExtensionArkPermission,
  assertExtensionEventPermission,
  assertExtensionHostPermission,
  type ExtensionSource as ExtensionPermissionSource,
} from "./extension-permissions";
import { loadRaycastPackageManifest, type RaycastPackageManifest } from "./raycast/manifest";

// ESM shim — __dirname / __filename не определены в Node ESM bundles.
const __filename = fileURLToPath(import.meta.url);
const __dirname = path.dirname(__filename);

export type ExtensionKind = "vue" | "static" | "native" | "raycast";

/**
 * Объявление команды в `manifest.json` extension'а. Полный id рендерится
 * как `${extension.id}:${command.id}`.
 */
export interface KextManifestCommand {
  /** Локальный id (без префикса extension'а). [a-z0-9:-]+. */
  id: string;
  /** Подпись в launcher'е (RU). */
  title: string;
  /** Доп. подпись справа (название extension'а или категория). */
  subtitle?: string;
  /** Иконка. Относительный путь от extension dir (например `icons/today.svg`).
      Если не задано — используется top-level `manifest.icon`. */
  icon?: string;
  /** Hash-route для openExtension. Передаётся в Eden через
      `kepler.navigation.initialRoute` / `onNavigate`. */
  route?: string;
  /** UI-классификация плашки. `app` = главное приложение, `command` = команда.
      По умолчанию `command`. */
  kind?: "app" | "command";
  /** Поведение при invoke:
   *  - `open` (default): открыть extension с route. Не требует running state.
   *  - `action`: invoke в running extension через ARK commands bus. Если
   *    extension не запущен — Kepler auto-launch'ит и dispatch'ит после
   *    mount. */
  mode?: "open" | "action" | "raycast-view" | "raycast-no-view" | "raycast-menu-bar";
}

export interface ExtensionManifest {
  id: string;
  name: string;
  /**
   * Собственная версия extension'а (semver `MAJOR.MINOR.PATCH`). Показывается
   * в UI install dialog'а и Settings → Расширения. Используется backup-системой
   * (`extensions-backups/<id>/<timestamp>/`) для отката на прошлую версию.
   */
  version?: string;
  /**
   * Краткое описание extension'а (одна строка). Показывается в install dialog.
   */
  description?: string;
  /**
   * Автор / организация. Только информационно — code-signing нет.
   */
  author?: string;
  /**
   * Декларируемые permissions. Для user-installed extension'ов runtime
   * enforce'ит их в main-process IPC; repo-dev и bundled first-party copies
   * считаются trusted, чтобы встроенные приложения не дублировали broad caps.
   */
  permissions?: string[];
  /**
   * Semver-range той версии Kepler API, на которой extension работает.
   * Например `"^1.0.0"` — accept любые 1.x.y версии, отвергнуть 2.0.0.
   * Если поле отсутствует — extension считается legacy и грузится без
   * проверки (warning в console). Рекомендуется всегда указывать.
   *
   * См. [`KEPLER_API_VERSION`](./kepler-api.ts) — текущая версия shell'а.
   */
  keplerApiVersion?: string;
  kind?: ExtensionKind;
  /**
   * Raycast-compatible package metadata derived from `package.json`.
   * Present only for `kind: "raycast"` extensions.
   */
  raycast?: RaycastPackageManifest;
  entryHtml?: string;
  /**
   * Native extension entrypoint. Used only when `kind: "native"`.
   *
   * `executable` is relative to extension dir for installed/bundled copies.
   * `devExecutable` is optional and used from repo dev tree when it exists
   * (for example `../../target/debug/my-native-app.exe`).
   * `args` are appended before shell-provided metadata args.
   */
  native?: {
    executable: string;
    devExecutable?: string;
    cargoPackage?: string;
    args?: string[];
    singleInstance?: boolean;
  };
  /**
   * Путь к preload-скрипту:
   *  - для "vue" по умолчанию используется shared preload из dist-electron;
   *  - для "static" значение интерпретируется относительно директории
   *    extension'а (legacy PoC mode).
   *  - специальное значение "kepler-extension-preload.mjs" принудительно
   *    использует shared preload.
   */
  preload?: string;
  /**
   * Относительный путь к иконке extension'а внутри его директории
   * (обычно `icon.png`). Используется launcher'ом для open-команд.
   */
  icon?: string;
  /**
   * Порт Vite dev server'а в developer mode. Если задан и активирован
   * developer mode (env `KEPLER_DEV=1` или toggle в Settings) — extension
   * грузится с `http://localhost:<devPort>/` вместо `dist/index.html`.
   */
  devPort?: number;
  width?: number;
  height?: number;
  minWidth?: number;
  minHeight?: number;
  /**
   * Windows backdrop material для окна. Если задан, BrowserWindow создаётся
   * с прозрачным backgroundColor и Win32 system backdrop. Renderer должен
   * иметь semi-transparent body, иначе эффект не виден.
   *
   * - `"acrylic"` — blur с лёгкой прозрачностью (Win11/10)
   * - `"mica"` — desktop tint Win11
   * - `"none"` — стандартный непрозрачный фон (по умолчанию)
   */
  windowEffect?: "acrylic" | "mica" | "none";
  /**
   * Если `true` — клик на «X» / Alt+F4 / `kepler:extension:window:close`
   * **скрывает** окно вместо destroy. Renderer остаётся жив (вместе со всеми
   * таймерами, ARK подписками и side-effect'ами), reopen через `openExtension`
   * мгновенно показывает hidden окно. Используется для extensions с долго
   * живущим процессом. Окно реально уничтожается только на quit приложения.
   *
   * Default: false (классический destroy on close).
   *
   * NB: окно ВСЁ ЕЩЁ показывается / прячется в taskbar при hide — пользователь
   * видит что extension "закрыт"; индикатор того что pomodoro продолжается —
   * floating focus widget.
   */
  keepAliveInBackground?: boolean;
  /**
   * Opt-in к `backgroundThrottling: false` для extension-owned renderer'а.
   *
   * По умолчанию (`false`) Chromium throttle'ит фоновые timers/rAF — безопасный
   * baseline, hidden/свёрнутые extension windows не жгут CPU попусту.
   *
   * `true` разрешается только extensions, чья логика **обоснованно** зависит
   * от foreground-accurate timers в скрытом окне (например, real-time sync-loop
   * без backend). Фоновые side-effect'ы лучше держать в backend/main/service.
   *
   * Default: false (throttling включён).
   */
  backgroundExecution?: boolean;
  /**
   * Declarative commands extension'а (Raycast-style). Manifest = source of
   * truth для entry-point команд: открыть extension с конкретным route,
   * либо триггернуть action который extension обработает через
   * `kepler.navigation.onNavigate`. Видны в launcher всегда (пока
   * extension установлен), не требуют running state.
   *
   * Полный id команды = `${manifest.id}:${cmd.id}` — security boundary
   * (extension не может claim'нуть чужой namespace, например `settings:open`).
   *
   * Подробнее — `docs-site/concepts/command-bus.md`.
   */
  commands?: KextManifestCommand[];
  /**
   * Test contract — опционально. Используется universal `tests/e2e/extensions-contract.spec.ts`
   * чтобы автоматически проверять архитектурный baseline extension'а: команды
   * appear в `commands.list` после boot'а, ARK smoke round-trip по объявленному
   * object type работает. Per-app UI flow'ы — отдельные spec'и.
   */
  tests?: {
    /**
     * Список command id, которые extension обязан зарегистрировать через
     * `commands.register` к моменту первого render. Universal contract spec
     * после boot'а делает `commands.list` и сравнивает.
     */
    commands?: string[];
    /**
     * ARK round-trip smoke: type id + (опционально) sample payload. Spec
     * делает `upsert_object` → `get_object` → `delete_object`.
     */
    smoke?: {
      objectType: string;
      sample?: {
        title?: string;
        content?: unknown;
        props?: Record<string, unknown>;
      };
    };
  };
}

interface ExtensionWindowEntry {
  win: BrowserWindow;
  id: string;
  /** Если true — close скрывает окно вместо destroy (см. ExtensionManifest.keepAliveInBackground). */
  keepAliveInBackground: boolean;
  /** Initial route переданный в `openExtension(id, route)` для cold start.
      Renderer читает через `kepler.navigation.initialRoute()` на mount. */
  initialRoute?: string;
}

const extensionWindows = new Map<string, ExtensionWindowEntry>();
interface NativeExtensionEntry {
  child: ChildProcess;
  id: string;
}

const nativeExtensions = new Map<string, NativeExtensionEntry>();

// При quit'е приложения keepAliveInBackground extension'ы должны реально
// уничтожиться, а не зацикливать preventDefault → app.exit вис. before-quit
// взводит этот флаг до того как BrowserWindow'ы получат close.
let isAppQuitting = false;
app.on("before-quit", () => {
  isAppQuitting = true;
});
interface ExtensionRendererContext {
  id: string;
  source: ExtensionPermissionSource;
  manifestPermissions?: readonly string[];
}

// Reverse map: webContents.id → extension context. Нужен, чтобы из IPC handler'а
// определить, какое окно отправило запрос, и какие permissions были у кода при
// открытии. Source snapshot важен: user-installed override нельзя доверять по id.
const webContentsToExtensionContext = new Map<number, ExtensionRendererContext>();

// ArkClient injected lazily из main.ts через `setArkClient`. Если null —
// extension'ы получают ошибку при попытке ARK-запроса.
type ArkRequestFn = (req: Record<string, unknown>) => Promise<unknown>;
type ArkSubscribeFn = (event: string, handler: (payload: unknown) => void) => () => void;

let arkRequest: ArkRequestFn | null = null;
let arkSubscribe: ArkSubscribeFn | null = null;

// Ready-gate для extension ARK bridge. Extension windows могут открыться раньше,
// чем main.ts успеет вызвать `setExtensionArkBridge(...)` после handshake'а
// ArkClient'а. Если в этот момент extension probe'нет ARK в onMounted,
// он получит «ark bridge not ready» и UI запомнит status=error
// до следующего probe-интервала (10s) — отсюда «горит индикатор не подключено».
//
// Решение: handler не throws сразу, а await'ит resolve этого promise (с
// timeout'ом), точно так же как `awaitArkReady()` для `kepler:ark:request`
// в main.ts. Reset (`setExtensionArkBridge({request: null, ...})` при shutdown)
// создаёт новый pending promise — следующие запросы зависнут до нового resolve
// или timeout'нутся.
let arkBridgeReady!: Promise<void>;
let arkBridgeReadyResolve: (() => void) | null = null;
function resetArkBridgeReady(): void {
  arkBridgeReady = new Promise<void>((resolve) => {
    arkBridgeReadyResolve = resolve;
  });
}
resetArkBridgeReady();

async function awaitArkBridgeReady(timeoutMs = 15000): Promise<void> {
  if (arkRequest) return;
  await Promise.race([
    arkBridgeReady,
    new Promise<void>((_, rej) =>
      setTimeout(() => rej(new Error("ark bridge not ready (timeout)")), timeoutMs),
    ),
  ]);
}

/**
 * Регистрируется из main.ts после init ArkClient'а. extension-host остаётся
 * loosely-coupled — не импортирует ArkClient напрямую и не дублирует логику
 * выбора self-managed / kepler-managed режима.
 *
 * Когда `opts.request` non-null — bridge переходит в ready-состояние, и все
 * pending IPC-запросы из extension'ов resume'ятся. Когда null (shutdown) —
 * bridge возвращается в not-ready, новые запросы будут ждать следующего init.
 */
export function setExtensionArkBridge(opts: {
  request: ArkRequestFn | null;
  subscribe: ArkSubscribeFn | null;
}): void {
  arkRequest = opts.request;
  arkSubscribe = opts.subscribe;
  if (opts.request) {
    arkBridgeReadyResolve?.();
  } else {
    resetArkBridgeReady();
  }
}

// ---------------------------------------------------------------------------
// Developer mode
// ---------------------------------------------------------------------------

/**
 * Читает `developerMode: boolean` из `<userData>/kepler-shell-settings.json`.
 * Если файла нет / повреждён — возвращает false. Sync read: вызывается
 * редко (на open extension), значит выигрыша от async нет.
 */
export function readDevModeSetting(): boolean {
  try {
    const file = path.join(keplerDataDir(), "kepler-shell-settings.json");
    if (!existsSync(file)) return false;
    const json = JSON.parse(readFileSync(file, "utf8")) as {
      developerMode?: boolean;
    };
    return !!json.developerMode;
  } catch {
    return false;
  }
}

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

function isShellInDevSession(): boolean {
  // VITE_DEV_SERVER_URL выставляется vite-plugin-electron только в dev session.
  // В packaged production его нет → probe не делаем, всегда dist.
  return !!process.env.VITE_DEV_SERVER_URL;
}

interface ExtensionSource {
  kind: "dev-server" | "dist";
  url?: string;
  file?: string;
}

async function resolveExtensionSource(
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

// Priority chain для resolution extension-папок. Higher priority first.
//
//   1. Dev source tree (`<repoRoot>/extensions/`) — если папка
//      существует. Это означает, что мы запущены из repo (developer flow).
//   2. User-installed (`%APPDATA%\Kosmos\extensions\<id>\`) — основной канал
//      для prod: пользователь устанавливает / обновляет extension через CLI
//      или (в будущем) через UI, копия живёт в writable location.
//   3. Bundled (`<resourcesPath>/extensions/<id>\`) — fallback для packaged
//      сборок: built-in extensions едут с Kepler installer'ом, user-installed
//      их перекрывает, удаление user-папки откатывает на bundled.
//
interface ExtensionRootEntry {
  dir: string;
  source: ExtensionPermissionSource;
}

// Per-id lookup (`resolveExtensionLocation`) обходит цепочку и возвращает первый
// корень, где есть `manifest.json`. Это позволяет смешивать user-installed
// и bundled extensions.
function resolveExtensionRootEntries(): ExtensionRootEntry[] {
  const roots: ExtensionRootEntry[] = [];
  // Repo dev tree: __dirname is platform/desktop/electron/ (or dist-electron/).
  // Source packages can live under products/*, incubator/*, or the deprecated
  // extensions/* compatibility root. User-installed runtime extensions still
  // live in <dataDir>/extensions and are handled below.
  const repoRoot = path.resolve(__dirname, "..", "..", "..");
  for (const rootName of ["products", "incubator", "extensions"]) {
    const dev = path.join(repoRoot, rootName);
    if (existsSync(dev)) roots.push({ dir: dev, source: "dev" });
  }
  const userRoot = path.join(keplerDataDir(), "extensions");
  if (!roots.some((root) => root.dir === userRoot)) roots.push({ dir: userRoot, source: "user" });
  if (process.resourcesPath) {
    const bundled = path.join(process.resourcesPath, "extensions");
    if (!roots.some((root) => root.dir === bundled))
      roots.push({ dir: bundled, source: "bundled" });
  }
  return roots;
}

function resolveExtensionRoots(): string[] {
  return resolveExtensionRootEntries().map((root) => root.dir);
}

/**
 * Path к директории, куда CLI installer пишет user-installed extensions.
 * Экспортируется, чтобы install/uninstall scripts могли импортировать его
 * (а не дублировать path-логику).
 */
export function userExtensionsRoot(): string {
  return path.join(keplerDataDir(), "extensions");
}

/**
 * Path к директории с persistent user data extension'а. Отдельный от code dir
 * (`extensions/<id>/`) — install/uninstall кода не трогают эту папку.
 *
 * Структура:
 *   <APPDATA>/Kosmos/extensions-data/<id>/
 *     settings.json        (опциональный, extension пишет через preload API)
 *     window-state.json    (Kepler shell пишет сам по window events)
 *     ...                  (любые user files extension'а)
 */
export function extensionUserDataDir(id: string): string {
  return path.join(keplerDataDir(), "extensions-data", id);
}

// Path traversal protection: name должно быть «нормальным» basename'ом —
// никаких `/`, `\`, `..`, не начинается с `.`. Используется во всех
// userData handler'ах перед join'ом с user data dir.
const USER_DATA_NAME_RE = /^[\w][\w.-]*$/;
const USER_DATA_PATH_SEGMENT_RE = /^[\w][\w.-]*$/;

function assertSafeUserDataName(name: unknown): asserts name is string {
  if (typeof name !== "string" || !USER_DATA_NAME_RE.test(name)) {
    throw new Error(`[kepler-shell] invalid user data file name: ${String(name)}`);
  }
}

function resolveSafeUserDataPath(dir: string, name: unknown): string {
  if (typeof name !== "string") {
    throw new Error(`[kepler-shell] invalid user data path: ${String(name)}`);
  }
  const normalized = name.replace(/\\/g, "/");
  if (!normalized || path.isAbsolute(normalized) || normalized.includes("\0")) {
    throw new Error(`[kepler-shell] invalid user data path: ${name}`);
  }
  const parts = normalized.split("/");
  if (
    parts.some((part) => part === "." || part === ".." || !USER_DATA_PATH_SEGMENT_RE.test(part))
  ) {
    throw new Error(`[kepler-shell] invalid user data path: ${name}`);
  }

  const resolvedDir = path.resolve(dir);
  const resolvedPath = path.resolve(resolvedDir, ...parts);
  const relative = path.relative(resolvedDir, resolvedPath);
  if (relative.startsWith("..") || path.isAbsolute(relative)) {
    throw new Error(`[kepler-shell] invalid user data path: ${name}`);
  }
  return resolvedPath;
}

function ensureUserDataDir(extId: string): string {
  const dir = extensionUserDataDir(extId);
  mkdirSync(dir, { recursive: true });
  return dir;
}

function resolveExtensionLocation(
  id: string,
): { dir: string; source: ExtensionPermissionSource } | null {
  for (const root of resolveExtensionRootEntries()) {
    const dir = path.join(root.dir, id);
    if (existsSync(path.join(dir, "manifest.json")) || existsSync(path.join(dir, "package.json"))) {
      return { dir, source: root.source };
    }
  }
  return null;
}

function resolveExtensionDir(id: string): string | null {
  return resolveExtensionLocation(id)?.dir ?? null;
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
    const raycast = loadRaycastPackageManifest(dir);
    if (!raycast) return null;
    if (raycast.name !== id) {
      console.warn(
        `[kepler-shell] Raycast package name mismatch: folder=${id}, package=${raycast.name}`,
      );
      return null;
    }
    return {
      id: raycast.name,
      name: raycast.title,
      version: raycast.version,
      description: raycast.description,
      author: raycast.author,
      permissions: raycast.kosmos?.permissions,
      kind: "raycast",
      icon: raycast.icon,
      windowEffect: raycast.kosmos?.windowEffect,
      keplerApiVersion: raycast.kosmos?.minKosmosApiVersion,
      raycast,
      commands: raycast.commands.map((command) => ({
        id: command.name,
        title: command.title,
        subtitle: command.subtitle ?? raycast.title,
        icon: command.icon ?? raycast.icon,
        kind: "command",
        mode:
          command.mode === "no-view"
            ? "raycast-no-view"
            : command.mode === "menu-bar"
              ? "raycast-menu-bar"
              : "raycast-view",
      })),
    };
  }
  try {
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

// In-memory cache иконок с mtime-инвалидацией — если файл иконки изменился
// на диске (user добавил новую), кеш автоматически перечитает на следующий
// запрос. Это важно для dev workflow когда иконки меняются в running session.
interface IconCacheEntry {
  uri: string | null;
  mtimeMs: number;
}
const iconDataUriCache = new Map<string, IconCacheEntry>();

/**
 * Возвращает icon extension'а как `data:image/png;base64,...` URI, или undefined
 * если у extension'а нет icon (нет поля в manifest или файл отсутствует).
 * Кеширует по mtime файла — повторные вызовы дешёвые, обновление файла
 * автоматически перечитывается.
 */
export function extensionIconDataUri(id: string): string | undefined {
  const manifest = loadExtensionManifest(id);
  if (!manifest || !manifest.icon) return undefined;
  const dir = resolveExtensionDir(id);
  if (!dir) return undefined;
  const iconPath = path.resolve(path.join(dir, manifest.icon));
  if (!iconPath.startsWith(path.resolve(dir) + path.sep) && iconPath !== path.resolve(dir))
    return undefined;
  if (!existsSync(iconPath)) return undefined;
  const stat = statSync(iconPath);
  const cached = iconDataUriCache.get(id);
  if (cached && cached.mtimeMs === stat.mtimeMs) {
    return cached.uri ?? undefined;
  }
  try {
    const buf = readFileSync(iconPath);
    const ext = path.extname(iconPath).toLowerCase();
    const mime =
      ext === ".svg"
        ? "image/svg+xml"
        : ext === ".jpg" || ext === ".jpeg"
          ? "image/jpeg"
          : "image/png";
    const uri = `data:${mime};base64,${buf.toString("base64")}`;
    iconDataUriCache.set(id, { uri, mtimeMs: stat.mtimeMs });
    return uri;
  } catch (e) {
    console.error(`[kepler-shell] failed to read icon for ${id}:`, e);
    iconDataUriCache.set(id, { uri: null, mtimeMs: stat.mtimeMs });
    return undefined;
  }
}

/**
 * Резолвит произвольный icon-path из манифеста (относительно extension dir)
 * в data:URI. Используется для `commands[].icon`. Возвращает undefined
 * если файл отсутствует / выходит за пределы extension dir / unreadable.
 */
function readManifestIconAsDataUri(id: string, iconRel: string): string | undefined {
  if (!iconRel || typeof iconRel !== "string") return undefined;
  const dir = resolveExtensionDir(id);
  if (!dir) return undefined;
  const resolved = path.resolve(path.join(dir, iconRel));
  // Path traversal guard: icon должна оставаться внутри extension dir.
  const dirResolved = path.resolve(dir);
  if (!resolved.startsWith(dirResolved + path.sep) && resolved !== dirResolved) return undefined;
  if (!existsSync(resolved)) return undefined;
  try {
    const buf = readFileSync(resolved);
    const ext = path.extname(resolved).toLowerCase();
    const mime =
      ext === ".svg"
        ? "image/svg+xml"
        : ext === ".jpg" || ext === ".jpeg"
          ? "image/jpeg"
          : ext === ".webp"
            ? "image/webp"
            : "image/png";
    return `data:${mime};base64,${buf.toString("base64")}`;
  } catch {
    return undefined;
  }
}

/**
 * Manifest-declared command, обогащённая resolved icon data:URI + metadata
 * extension'а. Этот формат уже близок к `CommandRecord` из ipc-types.ts.
 */
export interface DeclaredCommand {
  id: string;
  title: string;
  subtitle?: string;
  category: "open" | "action";
  kind: "app" | "command";
  appName: string;
  icon?: string;
  /** Source extension id (для exec). */
  extensionId: string;
  /** Hash-route из manifest (если задан). */
  route?: string;
  /** Mode: `open`, `action`, or Raycast-compatible command runner. */
  mode: "open" | "action" | "raycast-view" | "raycast-no-view" | "raycast-menu-bar";
  /** Raycast command name for `kind: "raycast"` packages. */
  raycastCommandName?: string;
}

const declaredCommandIdRe = /^[a-z0-9][a-z0-9:_-]*$/;

/**
 * Сканирует все установленные extension'ы (включая dev tree), читает
 * `manifest.commands[]`, билдит DeclaredCommand[]. Idempotent / lightweight —
 * можно дёргать на каждом `kepler:commands:list`. Cache не нужен потому что
 * `listExtensions()` уже relatively cheap (несколько readFileSync).
 */
export function loadDeclaredCommands(): DeclaredCommand[] {
  const out: DeclaredCommand[] = [];
  for (const manifest of listExtensions()) {
    if (!Array.isArray(manifest.commands)) continue;
    for (const cmd of manifest.commands) {
      if (!cmd || typeof cmd !== "object") continue;
      if (typeof cmd.id !== "string" || !declaredCommandIdRe.test(cmd.id)) {
        console.warn(
          `[kepler-shell] extension '${manifest.id}' command id invalid: ${JSON.stringify(cmd.id)} — skipped`,
        );
        continue;
      }
      if (typeof cmd.title !== "string" || cmd.title.trim().length === 0) {
        console.warn(
          `[kepler-shell] extension '${manifest.id}' command '${cmd.id}' missing title — skipped`,
        );
        continue;
      }
      const fullId = `${manifest.id}:${cmd.id}`;
      const icon = cmd.icon
        ? readManifestIconAsDataUri(manifest.id, cmd.icon)
        : extensionIconDataUri(manifest.id);
      out.push({
        id: fullId,
        title: cmd.title,
        subtitle: cmd.subtitle ?? manifest.name,
        category: cmd.mode === "action" || cmd.mode === "raycast-no-view" ? "action" : "open",
        kind: cmd.kind ?? "command",
        appName: manifest.name,
        icon,
        extensionId: manifest.id,
        route: cmd.route,
        mode: cmd.mode ?? "open",
        raycastCommandName: manifest.kind === "raycast" ? cmd.id : undefined,
      });
    }
  }
  return out;
}

/**
 * Lookup конкретной DeclaredCommand по полному id (`<ext>:<cmd>`).
 * Используется `kepler:commands:invoke` для resolve'а нужного route +
 * extension id перед openExtension/commands.invoke.
 */
export function findDeclaredCommand(fullId: string): DeclaredCommand | null {
  for (const cmd of loadDeclaredCommands()) {
    if (cmd.id === fullId) return cmd;
  }
  return null;
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

function resolvePreloadForManifest(
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

function openIncompatibilityWindow(manifest: ExtensionManifest, reason: string): void {
  const html = `<!doctype html>
<html lang="ru">
<head>
  <meta charset="utf-8" />
  <title>Расширение несовместимо — ${escapeHtml(manifest.name)}</title>
  <style>
    :root { color-scheme: dark; }
    html,body { margin:0; padding:0; height:100%; background:#1a1a1a; color:#e6e6e6;
      font: 13px/1.5 system-ui, -apple-system, Segoe UI, sans-serif; }
    .wrap { padding: 28px 32px; max-width: 520px; margin: 0 auto; }
    h1 { font-size: 16px; font-weight: 600; margin: 0 0 12px; color: #ff9b8a; }
    p { margin: 0 0 12px; color: #cfcfcf; }
    code { background: #2a2a2a; padding: 1px 6px; border-radius: 4px; font-size: 12px; }
    button { margin-top: 14px; background:#2d2d2d; border:1px solid #3d3d3d; color:#e6e6e6;
      padding: 6px 14px; border-radius: 6px; font: inherit; }
    button:hover { background:#383838; }
  </style>
</head>
<body>
  <div class="wrap">
    <h1>Расширение несовместимо</h1>
    <p>${escapeHtml(reason)}</p>
    <p><strong>ID:</strong> <code>${escapeHtml(manifest.id)}</code></p>
    <p><strong>Версия расширения:</strong> <code>${escapeHtml(manifest.version ?? "не указана")}</code></p>
    <button onclick="window.close()">Закрыть</button>
  </div>
</body>
</html>`;
  const win = new BrowserWindow({
    width: 560,
    height: 320,
    title: `Расширение несовместимо — ${manifest.name}`,
    backgroundColor: "#1a1a1a",
    resizable: false,
    minimizable: false,
    maximizable: false,
    webPreferences: {
      contextIsolation: true,
      nodeIntegration: false,
    },
  });
  void win.loadURL(`data:text/html;charset=utf-8,${encodeURIComponent(html)}`);
}

function escapeHtml(s: string): string {
  return s
    .replace(/&/g, "&amp;")
    .replace(/</g, "&lt;")
    .replace(/>/g, "&gt;")
    .replace(/"/g, "&quot;")
    .replace(/'/g, "&#39;");
}

/** Is extension currently running (window exists, not destroyed)? */
export function isExtensionRunning(id: string): boolean {
  const native = nativeExtensions.get(id);
  if (native && !native.child.killed && native.child.exitCode === null) return true;
  const entry = extensionWindows.get(id);
  return !!entry && !entry.win.isDestroyed();
}

/**
 * Поднять уже существующее окно extension'а на передний план.
 *
 * Why not just `win.focus()`:
 *   - Если minimized — focus() на Windows не разворачивает окно (Electron
 *     возвращает focus IF taskbar отвечает; без restore() окно остаётся в трее).
 *   - Если hidden (`win.isVisible() === false`, например после `win.hide()`) —
 *     focus() no-op'ает; нужен `show()`.
 *   - Если Kepler не foreground-app (юзер invoke'нул через global hotkey из
 *     другого приложения) — Win32 запрещает forceForegroundWindow от
 *     non-foreground процесса. Стандартный workaround — toggle
 *     `setAlwaysOnTop(true) → setAlwaysOnTop(false)` за один tick: окно
 *     поднимается, затем флаг снимается, обычный z-order behavior сохраняется.
 *
 * Order matters: restore() → show() → AOT-toggle → focus(). Restore первым,
 * иначе show()/focus() могут опять прилипнуть к taskbar.
 */
function focusExistingExtensionWindow(win: BrowserWindow): void {
  if (win.isDestroyed()) return;
  // Headless / test mode: окна не показываем, Playwright работает через
  // webContents без paint'а.
  if (process.env.KOSMOS_HEADLESS === "1" || process.env.KOSMOS_TEST_MODE === "1") {
    return;
  }
  try {
    if (win.isMinimized()) win.restore();
    if (!win.isVisible()) win.show();
    // Win32 quirk — toggle AOT чтобы поднять окно на передний план даже когда
    // Kepler не foreground-app. На macOS/Linux обычно достаточно focus(), но
    // toggle безопасен (короткая вспышка AOT не визуально заметна).
    const wasAlwaysOnTop = win.isAlwaysOnTop();
    if (!wasAlwaysOnTop) {
      win.setAlwaysOnTop(true);
      win.setAlwaysOnTop(false);
    }
    win.focus();
    win.moveTop();
  } catch (e) {
    // Race: окно могло destroy'нуться между isDestroyed-check и операцией.
    console.warn("[kepler-shell] focusExistingExtensionWindow failed:", e);
  }
}

// Dedupe concurrent openExtension calls для одного и того же id. Без этого
// два быстрых invoke могли создать два BrowserWindow'а: между existing-check
// и `extensionWindows.set` теперь есть `await resolveExtensionSource` (probe).
const openInflight = new Map<string, Promise<void>>();

export function openExtension(id: string, route?: string): Promise<void> {
  const existing = openInflight.get(id);
  if (existing) return existing;
  const promise = openExtensionImpl(id, route).finally(() => {
    openInflight.delete(id);
  });
  openInflight.set(id, promise);
  return promise;
}

export function reloadExtensionWindow(id: string): boolean {
  const entry = extensionWindows.get(id);
  if (!entry || entry.win.isDestroyed()) return false;
  entry.win.webContents.reloadIgnoringCache();
  return true;
}

async function openExtensionImpl(id: string, route?: string): Promise<void> {
  const manifest = loadExtensionManifest(id);
  if (!manifest) {
    console.warn(`[kepler-shell] extension not found: ${id}`);
    return;
  }
  if (manifest.kind === "native") {
    await openNativeExtension(id, manifest, route);
    return;
  }
  if (manifest.kind === "raycast") {
    console.warn(
      `[kepler-shell] Raycast view commands are not implemented yet: ${id}${route ? ` (${route})` : ""}`,
    );
    return;
  }
  const existing = extensionWindows.get(id);
  if (existing && !existing.win.isDestroyed()) {
    if (process.env.KOSMOS_HEADLESS !== "1") {
      focusExistingExtensionWindow(existing.win);
    }
    if (route) {
      try {
        existing.win.webContents.send("kepler:extension:navigation", route);
      } catch {
        /* webContents could be torn down between isDestroyed-check и send */
      }
    }
    return;
  }
  // Kepler API compat check ДО window create — несовместимые extension'ы
  // не должны получать live preload bridge (могут сломать инвариант API).
  const incompat = checkApiCompat(manifest);
  if (incompat) {
    console.error(`[kepler-shell] ${incompat}`);
    openIncompatibilityWindow(manifest, incompat);
    return;
  }
  const location = resolveExtensionLocation(id);
  if (!location) {
    console.warn(`[kepler-shell] extension dir disappeared: ${id}`);
    return;
  }
  const extensionDir = location.dir;
  // Source resolution: dev-server (Vite HMR) или dist/. Probe-based, см.
  // resolveExtensionSource выше. null = ни Vite не отвечает, ни dist не
  // существует — открыть нечего, abort.
  const source = await resolveExtensionSource(id, manifest, extensionDir);
  if (!source) return;
  const display = screen.getPrimaryDisplay().workAreaSize;
  const defaultWidth = manifest.width ?? 1200;
  const defaultHeight = manifest.height ?? 800;
  const preload = resolvePreloadForManifest(manifest, extensionDir);

  // Restore window bounds из persistent user data, если есть и валидны.
  // Файл живёт в `extensions-data/<id>/window-state.json` — не трогается
  // install/uninstall'ом кода.
  const stateFile = path.join(extensionUserDataDir(id), "window-state.json");
  let savedState: {
    width?: number;
    height?: number;
    x?: number;
    y?: number;
    isMaximized?: boolean;
  } = {};
  if (existsSync(stateFile)) {
    try {
      savedState = JSON.parse(readFileSync(stateFile, "utf8")) as typeof savedState;
    } catch (e) {
      console.warn(`[kepler-shell] extension '${id}' window-state.json invalid, ignoring:`, e);
    }
  }

  // Bounds sanitization: окно должно попадать хотя бы частично в какой-то
  // подключённый display. Иначе пользователь отключил монитор, на котором
  // окно стояло, и без проверки оно окажется за пределами экранов.
  const displays = screen.getAllDisplays();
  const isVisibleOnAnyDisplay = (x: number, y: number, w: number, h: number): boolean =>
    displays.some((d) => {
      const wa = d.workArea;
      return x + w > wa.x && x < wa.x + wa.width && y + h > wa.y && y < wa.y + wa.height;
    });

  let width = defaultWidth;
  let height = defaultHeight;
  let initialX: number | undefined = Math.round((display.width - defaultWidth) / 2);
  let initialY: number | undefined = Math.round((display.height - defaultHeight) / 2);

  if (
    typeof savedState.width === "number" &&
    typeof savedState.height === "number" &&
    typeof savedState.x === "number" &&
    typeof savedState.y === "number" &&
    isVisibleOnAnyDisplay(savedState.x, savedState.y, savedState.width, savedState.height)
  ) {
    width = savedState.width;
    height = savedState.height;
    initialX = savedState.x;
    initialY = savedState.y;
  }

  // KOSMOS_HEADLESS=1 (test mode) — окна создаются с show:false и skipTaskbar.
  // Playwright всё равно может evaluate() и locator() работать через
  // webContents без visible render. См. tests/e2e/helpers/launch.ts.
  const headless = process.env.KOSMOS_HEADLESS === "1";

  // Windows backdrop material: manifest remains per-extension fallback, while
  // KOSMOS_WINDOW_EFFECTS is the global benchmark/runtime override.
  const manifestFallback: KosmosWindowMaterial =
    manifest.windowEffect === "acrylic" || manifest.windowEffect === "mica"
      ? manifest.windowEffect
      : "none";
  const backgroundMaterial = resolveWindowMaterial(manifestFallback);
  const wantsBackdrop = backgroundMaterial === "acrylic" || backgroundMaterial === "mica";

  const win = new BrowserWindow({
    width,
    height,
    minWidth: manifest.minWidth ?? 800,
    minHeight: manifest.minHeight ?? 600,
    x: initialX,
    y: initialY,
    show: !headless,
    skipTaskbar: headless,
    title: manifest.name,
    backgroundColor: wantsBackdrop ? "#00000000" : "#1a1a1a",
    ...backgroundMaterialOption(backgroundMaterial),
    // Native controls живут в titleBarOverlay; renderer рисует только drag-region
    // и учитывает safe-area через env(titlebar-area-*).
    frame: true,
    titleBarStyle: "hidden",
    titleBarOverlay: {
      color: "#00000000",
      symbolColor: "#f5f5f5",
      height: 40,
    },
    // macOS: vibrancy (виден для backdrop-окон) + центрированные traffic
    // lights под 40px titlebar (на Windows — no-op).
    ...macWindowChrome({ trafficLightY: 14 }),
    webPreferences: {
      preload,
      contextIsolation: true,
      nodeIntegration: false,
      // Throttling включён по умолчанию — скрытые окна не жгут CPU.
      // Opt-out только если extension явно декларирует backgroundExecution: true.
      backgroundThrottling: manifest.backgroundExecution !== true,
    },
  });

  applyWindowMaterial(win, backgroundMaterial, `extension '${id}'`);

  // Если в saved state окно было maximized — восстановим после ready-to-show.
  if (savedState.isMaximized) {
    win.once("ready-to-show", () => {
      if (!win.isDestroyed()) win.maximize();
    });
  }

  // Persist window bounds на disk. Debounced для resized/moved (часто), sync
  // для maximize/unmaximize/close (редко, важно поймать финальное состояние).
  const saveWindowState = (): void => {
    try {
      if (win.isDestroyed()) return;
      // getBounds возвращает текущие, а не «нормальные» bounds —
      // если окно maximized, мы НЕ хотим перезаписывать сохранённые normal
      // bounds maximized-размером. Используем getNormalBounds.
      const bounds = win.getNormalBounds();
      const state = {
        width: bounds.width,
        height: bounds.height,
        x: bounds.x,
        y: bounds.y,
        isMaximized: win.isMaximized(),
        savedAt: new Date().toISOString(),
      };
      const dir = ensureUserDataDir(id);
      writeFileSync(path.join(dir, "window-state.json"), JSON.stringify(state, null, 2), "utf8");
    } catch (e) {
      console.warn(`[kepler-shell] extension '${id}' save window-state failed:`, e);
    }
  };

  let saveTimer: ReturnType<typeof setTimeout> | null = null;
  const scheduleSave = (): void => {
    if (saveTimer) clearTimeout(saveTimer);
    saveTimer = setTimeout(saveWindowState, 500);
  };

  win.on("resized", scheduleSave);
  win.on("moved", scheduleSave);
  const broadcastMaximizedState = (): void => {
    if (win.isDestroyed()) return;
    try {
      win.webContents.send("kepler:extension:window:maximized-changed", win.isMaximized());
    } catch {
      // renderer may not be ready yet — ignore
    }
  };
  win.on("maximize", () => {
    saveWindowState();
    broadcastMaximizedState();
  });
  win.on("unmaximize", () => {
    saveWindowState();
    broadcastMaximizedState();
  });
  win.on("close", (e) => {
    if (saveTimer) {
      clearTimeout(saveTimer);
      saveTimer = null;
    }
    saveWindowState();
    // keepAliveInBackground: intercept close → hide. App.before-quit взводит
    // isAppQuitting → пропускаем интерсепт на quit'е, иначе app не сможет
    // выйти. Headless / test mode тоже даёт реальный close — Playwright
    // явно закрывает окна.
    if (manifest.keepAliveInBackground && !isAppQuitting && !headless && !win.isDestroyed()) {
      e.preventDefault();
      win.hide();
    }
  });

  // Capture webContents.id ДО регистрации listener'ов. После 'closed' event
  // BrowserWindow.webContents уже destroyed и обращение к нему кидает
  // "Object has been destroyed".
  const wcId = win.webContents.id;
  webContentsToExtensionContext.set(wcId, {
    id,
    source: location.source,
    manifestPermissions: manifest.permissions,
  });
  win.on("closed", () => {
    clearExtensionTitlebarHoverTracker(wcId);
    extensionWindowDrags.delete(wcId);
    webContentsToExtensionContext.delete(wcId);
    extensionWindows.delete(id);
  });
  extensionWindows.set(id, {
    win,
    id,
    keepAliveInBackground: manifest.keepAliveInBackground === true,
    initialRoute: route,
  });

  // После полной загрузки renderer'а отправляем initial route — extension
  // ловит через `kepler.navigation.onNavigate` и делает `router.push(route)`.
  if (route) {
    win.webContents.once("did-finish-load", () => {
      try {
        win.webContents.send("kepler:extension:navigation", route);
      } catch {
        /* окно могло быть закрыто во время загрузки */
      }
    });
  }

  // F12 toggles DevTools для extension window (без модификаторов).
  // try/catch — на случай race condition при закрытии окна, когда event ещё
  // в очереди, а webContents уже destroyed.
  win.webContents.on("before-input-event", (e, input) => {
    if (input.key === "F12" && !input.alt && !input.control && !input.shift && !input.meta) {
      e.preventDefault();
      try {
        win.webContents.toggleDevTools();
      } catch {
        /* webContents destroyed mid-flight — игнорируем. */
      }
    }
  });

  // Route доставляется через IPC после did-finish-load (см. выше) — это
  // работает с любым vue-router history mode (memory / hash / web).
  if (source.kind === "dev-server" && source.url) {
    console.log(`[kepler-shell] extension '${id}' dev mode → ${source.url}`);
    void win.loadURL(source.url);
    win.webContents.openDevTools({ mode: "detach" });
  } else if (source.file) {
    void win.loadFile(source.file);
  }
}

export function raycastRuntimeContext(id: string): {
  manifest: ExtensionManifest;
  dir: string;
  source: ExtensionPermissionSource;
} | null {
  const manifest = loadExtensionManifest(id);
  const location = resolveExtensionLocation(id);
  if (!manifest || !location || manifest.kind !== "raycast") return null;
  return { manifest, dir: location.dir, source: location.source };
}

function resolveNativeExecutable(manifest: ExtensionManifest, extensionDir: string): string | null {
  const native = manifest.native;
  if (!native?.executable) {
    console.warn(`[kepler-shell] native extension '${manifest.id}' has no native.executable`);
    return null;
  }
  const candidates = [native.devExecutable, native.executable].filter(
    (value): value is string => typeof value === "string" && value.length > 0,
  );
  for (const rel of candidates) {
    const resolved = path.resolve(extensionDir, rel);
    if (existsSync(resolved)) return resolved;
  }
  console.warn(
    `[kepler-shell] native extension '${manifest.id}' executable not found: ${candidates.join(", ")}`,
  );
  return null;
}

async function openNativeExtension(
  id: string,
  manifest: ExtensionManifest,
  route?: string,
): Promise<void> {
  const singleInstance = manifest.native?.singleInstance !== false;
  const existing = nativeExtensions.get(id);
  if (existing && existing.child.exitCode === null && !existing.child.killed) {
    return;
  }
  if (singleInstance && existing) {
    nativeExtensions.delete(id);
  }
  const incompat = checkApiCompat(manifest);
  if (incompat) {
    console.error(`[kepler-shell] ${incompat}`);
    openIncompatibilityWindow(manifest, incompat);
    return;
  }
  const extensionDir = resolveExtensionDir(id);
  if (!extensionDir) {
    console.warn(`[kepler-shell] extension dir disappeared: ${id}`);
    return;
  }
  const exe = resolveNativeExecutable(manifest, extensionDir);
  if (!exe) return;

  if (process.env.KOSMOS_HEADLESS === "1" || process.env.KOSMOS_TEST_MODE === "1") {
    console.log(`[kepler-shell] headless: skip native extension spawn '${id}'`);
    return;
  }

  const args = [
    ...(manifest.native?.args ?? []),
    "--kosmos-extension-id",
    id,
    "--kosmos-user-data-dir",
    extensionUserDataDir(id),
  ];
  const devSession = isShellInDevSession();
  if (devSession) args.push("--kosmos-dev-mode");
  if (route) args.push("--route", route);
  const child = spawn(exe, args, {
    cwd: path.dirname(exe),
    env: {
      ...process.env,
      KOSMOS_EXTENSION_DEV_MODE: devSession ? "1" : undefined,
    },
    stdio: "ignore",
    detached: false,
    windowsHide: false,
  });
  nativeExtensions.set(id, { child, id });
  child.once("exit", () => {
    const current = nativeExtensions.get(id);
    if (current?.child === child) nativeExtensions.delete(id);
  });
  child.once("error", (e) => {
    console.error(`[kepler-shell] native extension '${id}' failed:`, e);
    const current = nativeExtensions.get(id);
    if (current?.child === child) nativeExtensions.delete(id);
  });
}

function windowForSender(sender: WebContents): BrowserWindow | null {
  const win = BrowserWindow.fromWebContents(sender);
  return win && !win.isDestroyed() ? win : null;
}

type ExtensionWindowDragState = {
  startBounds: { x: number; y: number; width: number; height: number };
  startScreenX: number;
  startScreenY: number;
};

const extensionWindowDrags = new Map<number, ExtensionWindowDragState>();
const extensionTitlebarHoverTrackers = new Map<number, NodeJS.Timeout>();

function clearExtensionTitlebarHoverTracker(webContentsId: number): void {
  const interval = extensionTitlebarHoverTrackers.get(webContentsId);
  if (!interval) return;
  clearInterval(interval);
  extensionTitlebarHoverTrackers.delete(webContentsId);
}

function extensionIdForSender(sender: WebContents): string | null {
  return webContentsToExtensionContext.get(sender.id)?.id ?? null;
}

function extensionContextForSender(sender: WebContents): ExtensionRendererContext {
  const context = webContentsToExtensionContext.get(sender.id);
  if (!context) {
    throw new Error("[kepler-shell] sender is not an extension");
  }
  return context;
}

export function assertExtensionSenderHostPermission(
  sender: WebContents,
  capability:
    | "userData.read"
    | "userData.write"
    | "focus.control"
    | "markdownFiles.open"
    | "markdownFiles.save",
): void {
  const context = extensionContextForSender(sender);
  assertExtensionHostPermission({
    extensionId: context.id,
    source: context.source,
    manifestPermissions: context.manifestPermissions,
    capability,
  });
}

export function assertExtensionSenderHostPermissionIfExtension(
  sender: WebContents,
  capability:
    | "userData.read"
    | "userData.write"
    | "focus.control"
    | "markdownFiles.open"
    | "markdownFiles.save",
): void {
  const context = webContentsToExtensionContext.get(sender.id);
  if (!context) return;
  assertExtensionHostPermission({
    extensionId: context.id,
    source: context.source,
    manifestPermissions: context.manifestPermissions,
    capability,
  });
}

// ---------------------------------------------------------------------------
// IPC: list / open (kept from previous PoC contract)
// ---------------------------------------------------------------------------

ipcMain.handle("kepler:extension:list", () => listExtensions());
ipcMain.handle("kepler:extension:open", async (_e, id: string) => {
  await openExtension(id);
});

// ---------------------------------------------------------------------------
// IPC: ARK proxy — extension renderer → main → ArkClient
// ---------------------------------------------------------------------------

ipcMain.handle(
  "kepler:extension:ark:request",
  async (e, operation: string, params?: Record<string, unknown>) => {
    await awaitArkBridgeReady();
    if (!arkRequest) {
      throw new Error("ark bridge not ready");
    }
    const context = extensionContextForSender(e.sender);
    if (params && Object.prototype.hasOwnProperty.call(params, "operation")) {
      throw new Error("[kepler-shell] extension ARK params must not include operation");
    }
    await assertExtensionArkPermission({
      extensionId: context.id,
      source: context.source,
      manifestPermissions: context.manifestPermissions,
      operation,
      params,
      // См. postmortems.md § 2026-06-04: type-scoped delete needs the target
      // object's type before the write can be authorized.
      resolveObjectType: async (id) => {
        const object = (await arkRequest?.({ operation: "get_object", id })) as
          | { typeId?: unknown; type_id?: unknown }
          | null
          | undefined;
        const typeId = object?.typeId ?? object?.type_id;
        return typeof typeId === "string" ? typeId : null;
      },
    });
    const req: Record<string, unknown> = { ...params, operation };
    const result = await arkRequest(req);

    // Post-process: focus.set_active_state → spawn helper bin для модификации
    // hosts. Fire-and-forget — UI не блокируем на admin elevation prompt.
    if (operation === "focus.set_active_state" && arkRequest) {
      void onFocusStateChanged(params, arkRequest).catch((e) => {
        console.warn("[extension-host] focus block apply failed:", e);
      });
    }

    return result;
  },
);

async function onFocusStateChanged(
  params: Record<string, unknown> | undefined,
  request: (req: Record<string, unknown>) => Promise<unknown>,
): Promise<void> {
  const active = !!params?.active;
  const blocklistId = (params?.blocklist_id as string | null | undefined) ?? null;

  // Lazy import — avoid cycle на startup time.
  const { applyFocusBlock } = await import("./focus-block");

  if (!active) {
    await applyFocusBlock({ active: false, domains: [] });
    return;
  }

  if (!blocklistId) {
    // active=true но нет blocklist_id → nothing to block.
    await applyFocusBlock({ active: false, domains: [] });
    return;
  }

  // Resolve domains для blocklist_id. Use focus.resolve_blocklist_domains
  // которое разворачивает @-references (kind: "raw") — fallback на
  // list_blocklists raw domains если op не известна.
  try {
    let domains: string[] = [];
    try {
      const resolved = (await request({
        operation: "focus.resolve_blocklist_domains",
        id: blocklistId,
      })) as { domains?: string[] } | null;
      domains = resolved?.domains ?? [];
    } catch {
      // Older backend без resolve op — fallback на raw list.
      const resp = (await request({ operation: "focus.list_blocklists" })) as {
        blocklists?: Array<{ id: string; domains: string[] }>;
      } | null;
      const list = resp?.blocklists ?? [];
      const found = list.find((b) => b.id === blocklistId);
      domains = found?.domains ?? [];
    }
    await applyFocusBlock({ active: true, domains });
  } catch (e) {
    console.warn("[extension-host] focus resolve failed:", e);
    await applyFocusBlock({ active: false, domains: [] });
  }
}

// Extension subscribes; main forwards events to that extension's webContents.
// Channel name encodes event name so multiple subscriptions on the same
// webContents do not collide.
const extensionEventUnsubscribers = new Map<string, () => void>();

ipcMain.handle("kepler:extension:ark:subscribe", async (e, event: string) => {
  await awaitArkBridgeReady();
  if (!arkSubscribe) {
    throw new Error("ark bridge not ready");
  }
  const context = extensionContextForSender(e.sender);
  assertExtensionEventPermission({
    extensionId: context.id,
    source: context.source,
    manifestPermissions: context.manifestPermissions,
    event,
  });
  const sender = e.sender;
  const key = `${sender.id}:${event}`;
  if (extensionEventUnsubscribers.has(key)) {
    // Idempotent — повторная подписка no-op.
    return true;
  }
  const unsubscribe = arkSubscribe(event, (payload) => {
    if (!sender.isDestroyed()) {
      sender.send(`kepler:extension:ark:event:${event}`, payload);
    }
  });
  extensionEventUnsubscribers.set(key, unsubscribe);
  // Cleanup при закрытии renderer'а.
  sender.once("destroyed", () => {
    const u = extensionEventUnsubscribers.get(key);
    if (u) {
      u();
      extensionEventUnsubscribers.delete(key);
    }
  });
  return true;
});

ipcMain.handle("kepler:extension:ark:unsubscribe", (e, event: string) => {
  const key = `${e.sender.id}:${event}`;
  const unsubscribe = extensionEventUnsubscribers.get(key);
  if (unsubscribe) {
    unsubscribe();
    extensionEventUnsubscribers.delete(key);
  }
  return true;
});

// ---------------------------------------------------------------------------
// IPC: meta / window controls (extension renderer → main)
// ---------------------------------------------------------------------------

ipcMain.handle("kepler:extension:meta:id", (e) => extensionIdForSender(e.sender));

ipcMain.handle("kepler:extension:navigation:initial", (e): string | null => {
  const id = extensionIdForSender(e.sender);
  if (!id) return null;
  const entry = extensionWindows.get(id);
  return entry?.initialRoute ?? null;
});

ipcMain.handle("kepler:extension:window:close", (e) => {
  const win = windowForSender(e.sender);
  if (win) win.close();
});

ipcMain.handle("kepler:extension:window:minimize", (e) => {
  const win = windowForSender(e.sender);
  if (win) win.minimize();
});

ipcMain.handle("kepler:extension:window:maximize", (e) => {
  const win = windowForSender(e.sender);
  if (!win) return;
  if (win.isMaximized()) {
    win.unmaximize();
  } else {
    win.maximize();
  }
});

ipcMain.handle("kepler:extension:window:is-maximized", (e): boolean => {
  const win = windowForSender(e.sender);
  return win ? win.isMaximized() : false;
});

ipcMain.handle("kepler:extension:window:zoom-get", (e): number => {
  return e.sender.getZoomFactor();
});

ipcMain.handle("kepler:extension:window:zoom-set", (e, factor: number): number => {
  const next = typeof factor === "number" && Number.isFinite(factor) ? factor : 1;
  const clamped = Math.max(0.5, Math.min(2.0, next));
  e.sender.setZoomFactor(clamped);
  return clamped;
});

// Контроль возможности maximize для extension-окон.
ipcMain.handle("kepler:extension:window:set-maximizable", (e, value: boolean) => {
  const win = windowForSender(e.sender);
  if (!win || win.isDestroyed()) return;
  win.setMaximizable(Boolean(value));
});

ipcMain.handle("kepler:extension:window:set-titlebar-symbol-color", (e, symbolColor: string) => {
  const win = windowForSender(e.sender);
  if (!win || win.isDestroyed()) return;
  win.setTitleBarOverlay({
    color: "#00000000",
    symbolColor,
    height: 40,
  });
});

ipcMain.handle("kepler:extension:window:begin-manual-drag", (e, point) => {
  const win = windowForSender(e.sender);
  if (!win || win.isDestroyed()) return;
  if (!point || typeof point.screenX !== "number" || typeof point.screenY !== "number") return;

  if (win.isMaximized()) win.unmaximize();
  extensionWindowDrags.set(e.sender.id, {
    startBounds: win.getBounds(),
    startScreenX: point.screenX,
    startScreenY: point.screenY,
  });
});

ipcMain.handle("kepler:extension:window:move-manual-drag", (e, point) => {
  const win = windowForSender(e.sender);
  const drag = extensionWindowDrags.get(e.sender.id);
  if (!win || win.isDestroyed() || !drag) return;
  if (!point || typeof point.screenX !== "number" || typeof point.screenY !== "number") return;

  win.setBounds({
    ...drag.startBounds,
    x: Math.round(drag.startBounds.x + point.screenX - drag.startScreenX),
    y: Math.round(drag.startBounds.y + point.screenY - drag.startScreenY),
  });
});

ipcMain.handle("kepler:extension:window:end-manual-drag", (e) => {
  extensionWindowDrags.delete(e.sender.id);
});

ipcMain.handle("kepler:extension:window:set-titlebar-hover-tracking", (e, enabled, height) => {
  const win = windowForSender(e.sender);
  if (!win || win.isDestroyed()) return;

  const webContentsId = e.sender.id;
  clearExtensionTitlebarHoverTracker(webContentsId);

  if (!enabled) {
    e.sender.send("kepler:extension:window:titlebar-hover-changed", false);
    return;
  }

  const titlebarHeight = typeof height === "number" && height > 0 ? height : 56;
  let lastHovered: boolean | null = null;

  const interval = setInterval(() => {
    if (win.isDestroyed() || e.sender.isDestroyed()) {
      clearExtensionTitlebarHoverTracker(webContentsId);
      return;
    }

    const cursor = screen.getCursorScreenPoint();
    const bounds = win.getBounds();
    const hovered =
      cursor.x >= bounds.x &&
      cursor.x <= bounds.x + bounds.width &&
      cursor.y >= bounds.y &&
      cursor.y <= bounds.y + titlebarHeight;

    if (hovered === lastHovered) return;
    lastHovered = hovered;
    e.sender.send("kepler:extension:window:titlebar-hover-changed", hovered);
  }, 33);

  extensionTitlebarHoverTrackers.set(webContentsId, interval);
});

// "Dock corner" — toggle между floating-widget mode (always-on-top,
// compact size, top-right corner) и обычным окном (восстановленный bounds).
// Используется Eden в zen mode по двойному клику на titlebar — превращает
// дневник в всегда-видимый mini-widget. Повторный dblclick возвращает.
const dockedState = new Map<
  string,
  { x: number; y: number; width: number; height: number; alwaysOnTop: boolean }
>();
const DOCK_WIDTH = 360;
const DOCK_HEIGHT = 560;

function broadcastDocked(win: BrowserWindow, isDocked: boolean): void {
  if (win.isDestroyed()) return;
  try {
    win.webContents.send("kepler:extension:window:docked-changed", isDocked);
  } catch {
    // renderer not ready — ignore
  }
}

ipcMain.handle("kepler:extension:window:toggle-dock-corner", (e) => {
  const id = extensionIdForSender(e.sender);
  const win = windowForSender(e.sender);
  if (!id || !win || win.isDestroyed()) return;

  const stored = dockedState.get(id);
  if (stored) {
    // Undock: restore.
    win.setAlwaysOnTop(stored.alwaysOnTop);
    win.setBounds({
      x: stored.x,
      y: stored.y,
      width: stored.width,
      height: stored.height,
    });
    dockedState.delete(id);
    broadcastDocked(win, false);
    return;
  }

  // Dock: save current bounds + переместить в top-right того дисплея,
  // где сейчас окно (multi-monitor safe).
  const current = win.getNormalBounds();
  dockedState.set(id, {
    x: current.x,
    y: current.y,
    width: current.width,
    height: current.height,
    alwaysOnTop: win.isAlwaysOnTop(),
  });

  if (win.isMaximized()) win.unmaximize();
  const display = screen.getDisplayMatching(current);
  const workArea = display.workArea;
  // Margin'ы для зазора + компенсация Win11 invisible resize-borders
  // (`frame: true` + `titleBarStyle: "hidden"` рисует ~8px невидимых
  // borders, outer bounds setBounds их включает).
  const marginX = 12;
  const marginTop = 12;
  const width = Math.min(DOCK_WIDTH, workArea.width - marginX * 2);
  const height = Math.min(DOCK_HEIGHT, workArea.height - marginTop);
  win.setBounds({
    x: workArea.x + workArea.width - width - marginX,
    y: workArea.y + marginTop,
    width,
    height,
  });

  // Подстраховка: если actual bounds после setBounds всё равно вылезают
  // за правый край workArea (бывает при DPI scaling > 100% — Electron
  // округляет фрейм physical→DIP неточно), сдвинем влево на overflow.
  setTimeout(() => {
    if (win.isDestroyed()) return;
    const actual = win.getBounds();
    const overflow = actual.x + actual.width - (workArea.x + workArea.width);
    if (overflow > 0) {
      win.setBounds({ ...actual, x: actual.x - overflow - marginX });
    }
  }, 0);

  win.setAlwaysOnTop(true, "floating");
  broadcastDocked(win, true);
});

ipcMain.handle("kepler:extension:window:is-docked", (e): boolean => {
  const id = extensionIdForSender(e.sender);
  return id ? dockedState.has(id) : false;
});

// ---------------------------------------------------------------------------
// IPC: host-action (extension → kepler-shell host action)
// ---------------------------------------------------------------------------

// Reserved для будущих host-action типа "show settings", "focus launcher" и т.п.
// Сейчас просто логирует и возвращает false (action not handled).
ipcMain.handle("kepler:extension:invoke-host", (_e, action: string, _payload?: unknown) => {
  console.error(`[kepler-shell] extension invoke-host: ${action} (no handler)`);
  return false;
});

// ---------------------------------------------------------------------------
// IPC: Markdown file dialogs
// ---------------------------------------------------------------------------

const MARKDOWN_FILE_MAX_BYTES = 5 * 1024 * 1024;
const MARKDOWN_VAULT_MAX_FILES = 5_000;
const MARKDOWN_VAULT_MAX_IMAGES = 5_000;
const MARKDOWN_IMAGE_EXTENSIONS = new Set([".png", ".jpg", ".jpeg", ".gif", ".webp", ".avif"]);

type MarkdownVaultTextFile = {
  path: string;
  relativePath: string;
  name: string;
  content: string;
};

type MarkdownVaultImageFile = {
  path: string;
  relativePath: string;
  name: string;
  fileUrl: string;
  mimeType: string;
  sizeBytes: number;
  width: number | null;
  height: number | null;
};

type MarkdownVaultOpenResult = {
  rootPath: string;
  files: MarkdownVaultTextFile[];
  images: MarkdownVaultImageFile[];
};

type MarkdownVaultExportFile =
  | {
      relativePath: string;
      content: string;
      sourcePath?: never;
    }
  | {
      relativePath: string;
      sourcePath: string;
      content?: never;
    };

function markdownDialogParent(sender: WebContents): BrowserWindow | undefined {
  return BrowserWindow.fromWebContents(sender) ?? undefined;
}

function safeMarkdownDefaultName(name: unknown): string {
  const fallback = "eden-object.md";
  if (typeof name !== "string") return fallback;
  const base = path
    .basename(name)
    .replace(/[<>:"/\\|?*]/g, "-")
    .replace(/./g, (char) => (char.charCodeAt(0) < 32 ? "-" : char))
    .trim();
  if (!base) return fallback;
  return base.toLowerCase().endsWith(".md") ? base : `${base}.md`;
}

function normalizeVaultRelativePath(root: string, filePath: string): string {
  return path.relative(root, filePath).split(path.sep).join("/");
}

function isIgnoredVaultDir(name: string): boolean {
  return name.startsWith(".") || name === "node_modules";
}

function imageMimeType(filePath: string): string {
  const ext = path.extname(filePath).toLowerCase();
  if (ext === ".png") return "image/png";
  if (ext === ".jpg" || ext === ".jpeg") return "image/jpeg";
  if (ext === ".gif") return "image/gif";
  if (ext === ".webp") return "image/webp";
  if (ext === ".avif") return "image/avif";
  return "application/octet-stream";
}

function readUInt24LE(buffer: Buffer, offset: number): number {
  return buffer[offset] + (buffer[offset + 1] << 8) + (buffer[offset + 2] << 16);
}

function readImageDimensions(filePath: string): { width: number; height: number } | null {
  try {
    const buffer = readFileSync(filePath);
    if (
      buffer.length >= 24 &&
      buffer[0] === 0x89 &&
      buffer[1] === 0x50 &&
      buffer[2] === 0x4e &&
      buffer[3] === 0x47
    ) {
      return { width: buffer.readUInt32BE(16), height: buffer.readUInt32BE(20) };
    }

    if (buffer.length >= 10 && buffer.toString("ascii", 0, 6) === "GIF87a") {
      return { width: buffer.readUInt16LE(6), height: buffer.readUInt16LE(8) };
    }
    if (buffer.length >= 10 && buffer.toString("ascii", 0, 6) === "GIF89a") {
      return { width: buffer.readUInt16LE(6), height: buffer.readUInt16LE(8) };
    }

    if (buffer.length >= 12 && buffer[0] === 0xff && buffer[1] === 0xd8) {
      let offset = 2;
      while (offset + 9 < buffer.length) {
        if (buffer[offset] !== 0xff) {
          offset += 1;
          continue;
        }
        const marker = buffer[offset + 1];
        const size = buffer.readUInt16BE(offset + 2);
        if (size < 2) return null;
        if (
          (marker >= 0xc0 && marker <= 0xc3) ||
          (marker >= 0xc5 && marker <= 0xc7) ||
          (marker >= 0xc9 && marker <= 0xcb) ||
          (marker >= 0xcd && marker <= 0xcf)
        ) {
          return {
            height: buffer.readUInt16BE(offset + 5),
            width: buffer.readUInt16BE(offset + 7),
          };
        }
        offset += 2 + size;
      }
    }

    if (
      buffer.length >= 30 &&
      buffer.toString("ascii", 0, 4) === "RIFF" &&
      buffer.toString("ascii", 8, 12) === "WEBP"
    ) {
      const chunk = buffer.toString("ascii", 12, 16);
      if (chunk === "VP8X" && buffer.length >= 30) {
        return {
          width: readUInt24LE(buffer, 24) + 1,
          height: readUInt24LE(buffer, 27) + 1,
        };
      }
      if (chunk === "VP8 " && buffer.length >= 30) {
        return {
          width: buffer.readUInt16LE(26) & 0x3fff,
          height: buffer.readUInt16LE(28) & 0x3fff,
        };
      }
      if (chunk === "VP8L" && buffer.length >= 25) {
        const bits = buffer.readUInt32LE(21);
        return {
          width: (bits & 0x3fff) + 1,
          height: ((bits >> 14) & 0x3fff) + 1,
        };
      }
    }
  } catch {
    return null;
  }

  return null;
}

function scanMarkdownVault(rootPath: string): MarkdownVaultOpenResult {
  const files: MarkdownVaultTextFile[] = [];
  const images: MarkdownVaultImageFile[] = [];

  const visit = (dir: string) => {
    const entries = readdirSync(dir, { withFileTypes: true });
    for (const entry of entries) {
      const fullPath = path.join(dir, entry.name);
      if (entry.isDirectory()) {
        if (!isIgnoredVaultDir(entry.name)) visit(fullPath);
        continue;
      }
      if (!entry.isFile()) continue;

      const ext = path.extname(entry.name).toLowerCase();
      if (ext === ".md" || ext === ".markdown") {
        if (files.length >= MARKDOWN_VAULT_MAX_FILES) continue;
        const stat = statSync(fullPath);
        if (stat.size > MARKDOWN_FILE_MAX_BYTES) continue;
        files.push({
          path: fullPath,
          relativePath: normalizeVaultRelativePath(rootPath, fullPath),
          name: entry.name,
          content: readFileSync(fullPath, "utf8"),
        });
        continue;
      }

      if (MARKDOWN_IMAGE_EXTENSIONS.has(ext)) {
        if (images.length >= MARKDOWN_VAULT_MAX_IMAGES) continue;
        const stat = statSync(fullPath);
        const dimensions = readImageDimensions(fullPath);
        images.push({
          path: fullPath,
          relativePath: normalizeVaultRelativePath(rootPath, fullPath),
          name: entry.name,
          fileUrl: localImageUrl(fullPath),
          mimeType: imageMimeType(fullPath),
          sizeBytes: stat.size,
          width: dimensions?.width ?? null,
          height: dimensions?.height ?? null,
        });
      }
    }
  };

  visit(rootPath);

  return {
    rootPath,
    files,
    images,
  };
}

function safeVaultOutputPath(rootPath: string, relativePath: string): string {
  const normalizedRelative = relativePath.replace(/\\/g, "/");
  if (
    normalizedRelative.startsWith("/") ||
    normalizedRelative.includes("../") ||
    normalizedRelative === ".." ||
    /^[a-zA-Z]:/.test(normalizedRelative)
  ) {
    throw new Error("[kepler-shell] unsafe Markdown export relative path");
  }

  const outputPath = path.resolve(rootPath, normalizedRelative);
  const root = path.resolve(rootPath);
  if (outputPath !== root && !outputPath.startsWith(`${root}${path.sep}`)) {
    throw new Error("[kepler-shell] Markdown export path escapes output directory");
  }
  return outputPath;
}

function resolveMarkdownVaultSourcePath(sourcePath: string): string | null {
  const trimmed = sourcePath.trim();
  if (!trimmed) return null;

  if (/^file:/i.test(trimmed)) {
    try {
      const resolved = fileURLToPath(trimmed);
      return path.isAbsolute(resolved) ? resolved : null;
    } catch {
      return null;
    }
  }

  const localImagePath = parseLocalImageRequestUrl(trimmed);
  if (localImagePath) {
    return path.isAbsolute(localImagePath) ? localImagePath : null;
  }

  if (path.isAbsolute(trimmed)) {
    return path.resolve(trimmed);
  }

  return null;
}

ipcMain.handle(
  "kepler:extension:markdownFiles:open",
  async (e): Promise<{ path: string; name: string; content: string } | null> => {
    assertExtensionSenderHostPermission(e.sender, "markdownFiles.open");
    const parent = markdownDialogParent(e.sender);
    const options: OpenDialogOptions = {
      title: "Импорт Markdown",
      properties: ["openFile"],
      filters: [{ name: "Markdown", extensions: ["md", "markdown"] }],
    };
    const result = parent
      ? await dialog.showOpenDialog(parent, options)
      : await dialog.showOpenDialog(options);
    if (result.canceled || result.filePaths.length === 0) return null;

    const filePath = result.filePaths[0];
    const stat = statSync(filePath);
    if (!stat.isFile()) {
      throw new Error("[kepler-shell] Markdown import expects a file");
    }
    if (stat.size > MARKDOWN_FILE_MAX_BYTES) {
      throw new Error("[kepler-shell] Markdown file is too large");
    }

    return {
      path: filePath,
      name: path.basename(filePath),
      content: readFileSync(filePath, "utf8"),
    };
  },
);

ipcMain.handle(
  "kepler:extension:markdownFiles:openVault",
  async (e): Promise<MarkdownVaultOpenResult | null> => {
    assertExtensionSenderHostPermission(e.sender, "markdownFiles.open");
    const parent = markdownDialogParent(e.sender);
    const options: OpenDialogOptions = {
      title: "Импорт Obsidian vault",
      properties: ["openDirectory"],
    };
    const result = parent
      ? await dialog.showOpenDialog(parent, options)
      : await dialog.showOpenDialog(options);
    if (result.canceled || result.filePaths.length === 0) return null;

    const rootPath = result.filePaths[0];
    const stat = statSync(rootPath);
    if (!stat.isDirectory()) {
      throw new Error("[kepler-shell] Markdown vault import expects a directory");
    }

    return scanMarkdownVault(rootPath);
  },
);

ipcMain.handle(
  "kepler:extension:markdownFiles:save",
  async (e, suggestedName: unknown, content: unknown): Promise<{ path: string } | null> => {
    assertExtensionSenderHostPermission(e.sender, "markdownFiles.save");
    if (typeof content !== "string") {
      throw new Error("[kepler-shell] Markdown export content must be a string");
    }
    const parent = markdownDialogParent(e.sender);
    const options: SaveDialogOptions = {
      title: "Экспорт Markdown",
      defaultPath: safeMarkdownDefaultName(suggestedName),
      filters: [{ name: "Markdown", extensions: ["md"] }],
    };
    const result = parent
      ? await dialog.showSaveDialog(parent, options)
      : await dialog.showSaveDialog(options);
    if (result.canceled || !result.filePath) return null;

    writeFileSync(result.filePath, content, "utf8");
    return { path: result.filePath };
  },
);

ipcMain.handle(
  "kepler:extension:markdownFiles:exportVault",
  async (e, files: unknown): Promise<{ outputDir: string; exportedCount: number } | null> => {
    assertExtensionSenderHostPermission(e.sender, "markdownFiles.save");
    if (!Array.isArray(files)) {
      throw new Error("[kepler-shell] Markdown vault export files must be an array");
    }
    const parent = markdownDialogParent(e.sender);
    const options: OpenDialogOptions = {
      title: "Экспорт Eden в Obsidian vault",
      properties: ["openDirectory", "createDirectory"],
    };
    const result = parent
      ? await dialog.showOpenDialog(parent, options)
      : await dialog.showOpenDialog(options);
    if (result.canceled || result.filePaths.length === 0) return null;

    const outputDir = result.filePaths[0];
    const stat = statSync(outputDir);
    if (!stat.isDirectory()) {
      throw new Error("[kepler-shell] Markdown vault export expects a directory");
    }

    let exportedCount = 0;
    let skippedAssetCount = 0;
    const writtenPaths = new Set<string>();
    for (const file of files as MarkdownVaultExportFile[]) {
      if (!file || typeof file.relativePath !== "string") {
        continue;
      }

      const outputPath = safeVaultOutputPath(outputDir, file.relativePath);
      if (writtenPaths.has(outputPath)) {
        throw new Error("[kepler-shell] Markdown vault export path already exists in payload");
      }
      writtenPaths.add(outputPath);
      mkdirSync(path.dirname(outputPath), { recursive: true });

      if (typeof file.content === "string") {
        writeFileSync(outputPath, file.content, "utf8");
        exportedCount += 1;
        continue;
      }

      if (typeof file.sourcePath === "string") {
        const sourcePath = resolveMarkdownVaultSourcePath(file.sourcePath);
        if (!sourcePath) {
          skippedAssetCount += 1;
          console.warn(
            "[kepler-shell] Markdown vault export asset skipped: unsafe or unsupported source path",
            file.sourcePath,
          );
          continue;
        }

        try {
          const stat = statSync(sourcePath);
          if (!stat.isFile()) {
            skippedAssetCount += 1;
            console.warn(
              "[kepler-shell] Markdown vault export asset skipped: source is not a file",
              sourcePath,
            );
            continue;
          }

          copyFileSync(sourcePath, outputPath);
          exportedCount += 1;
        } catch (error) {
          skippedAssetCount += 1;
          console.warn(
            "[kepler-shell] Markdown vault export asset skipped: copy failed",
            sourcePath,
            error,
          );
        }
        continue;
      }
    }

    if (skippedAssetCount > 0) {
      console.warn(
        `[kepler-shell] Markdown vault export completed with ${skippedAssetCount} skipped asset(s)`,
      );
    }

    return { outputDir, exportedCount };
  },
);

// ---------------------------------------------------------------------------
// IPC: userData (extension renderer → main → <APPDATA>/Kosmos/extensions-data/<id>/)
// ---------------------------------------------------------------------------
//
// Persistent user data extension'а — settings.json и любые другие files,
// которые extension хочет хранить локально (а не в ARK). Path физически
// отделён от code dir (`extensions/<id>/`), поэтому install/uninstall кода
// не трогает эти файлы. Каждый handler определяет extension id по sender —
// extension не может писать в чужой namespace.

function senderUserDataDir(sender: WebContents): string {
  const context = extensionContextForSender(sender);
  return ensureUserDataDir(context.id);
}

ipcMain.handle("kepler:extension:userData:path", (e) => {
  assertExtensionSenderHostPermission(e.sender, "userData.read");
  return senderUserDataDir(e.sender);
});

ipcMain.handle("kepler:extension:userData:readJson", (e, name: string): unknown => {
  assertExtensionSenderHostPermission(e.sender, "userData.read");
  assertSafeUserDataName(name);
  const dir = senderUserDataDir(e.sender);
  const filePath = path.join(dir, name);
  if (!existsSync(filePath)) return null;
  try {
    return JSON.parse(readFileSync(filePath, "utf8")) as unknown;
  } catch (err) {
    console.warn(`[kepler-shell] userData.readJson failed for ${filePath}:`, err);
    return null;
  }
});

ipcMain.handle("kepler:extension:userData:writeJson", (e, name: string, value: unknown): void => {
  assertExtensionSenderHostPermission(e.sender, "userData.write");
  assertSafeUserDataName(name);
  const dir = senderUserDataDir(e.sender);
  const filePath = path.join(dir, name);
  writeFileSync(filePath, JSON.stringify(value, null, 2), "utf8");
});

ipcMain.handle("kepler:extension:userData:readFile", (e, name: string): string | null => {
  assertExtensionSenderHostPermission(e.sender, "userData.read");
  assertSafeUserDataName(name);
  const dir = senderUserDataDir(e.sender);
  const filePath = path.join(dir, name);
  if (!existsSync(filePath)) return null;
  try {
    return readFileSync(filePath, "utf8");
  } catch (err) {
    console.warn(`[kepler-shell] userData.readFile failed for ${filePath}:`, err);
    return null;
  }
});

ipcMain.handle("kepler:extension:userData:writeFile", (e, name: string, content: string): void => {
  assertExtensionSenderHostPermission(e.sender, "userData.write");
  assertSafeUserDataName(name);
  if (typeof content !== "string") {
    throw new Error("[kepler-shell] userData.writeFile: content must be a string");
  }
  const dir = senderUserDataDir(e.sender);
  const filePath = path.join(dir, name);
  writeFileSync(filePath, content, "utf8");
});

ipcMain.handle("kepler:extension:userData:readBinary", (e, name: string): string | null => {
  assertExtensionSenderHostPermission(e.sender, "userData.read");
  const dir = senderUserDataDir(e.sender);
  const filePath = resolveSafeUserDataPath(dir, name);
  if (!existsSync(filePath)) return null;
  try {
    return readFileSync(filePath).toString("base64");
  } catch (err) {
    console.warn(`[kepler-shell] userData.readBinary failed for ${filePath}:`, err);
    return null;
  }
});

ipcMain.handle("kepler:extension:userData:writeBinary", (e, name: string, base64: string): void => {
  assertExtensionSenderHostPermission(e.sender, "userData.write");
  if (typeof base64 !== "string") {
    throw new Error("[kepler-shell] userData.writeBinary: content must be a base64 string");
  }
  const dir = senderUserDataDir(e.sender);
  const filePath = resolveSafeUserDataPath(dir, name);
  mkdirSync(path.dirname(filePath), { recursive: true });
  writeFileSync(filePath, Buffer.from(base64, "base64"));
});

ipcMain.handle("kepler:extension:userData:deleteFile", (e, name: string): boolean => {
  assertExtensionSenderHostPermission(e.sender, "userData.write");
  const dir = senderUserDataDir(e.sender);
  const filePath = resolveSafeUserDataPath(dir, name);
  if (!existsSync(filePath)) return false;
  rmSync(filePath, { force: true });
  return true;
});

// ---------------------------------------------------------------------------
// IPC: .kext install / preview / list / revert / uninstall
// ---------------------------------------------------------------------------
//
// Используется install dialog (`#install-extension`) + Settings → Расширения.
// install:preview — читает .kext без extract'а, возвращает manifest preview;
// install:do — собственно установка с backup'ом;
// list — installed user-extensions для Settings UI;
// revert — восстановить из backup'а;
// uninstall — удалить user copy.

ipcMain.handle("kepler:extension:install:preview", async (_e, sourcePath: string) => {
  if (typeof sourcePath !== "string") {
    throw new Error("install:preview: sourcePath must be a string");
  }
  return await previewSource(sourcePath);
});

/**
 * Broadcast `kepler:commands:updated` ко всем BrowserWindow'ам — launcher,
 * settings и т.п. Чтобы их UI перефетчил commands list после install /
 * uninstall (static команды фильтруются по тому, какие extensions
 * установлены — см. `commands.ts` `requiresExtension`).
 */
function notifyCommandsChanged(): void {
  for (const win of BrowserWindow.getAllWindows()) {
    if (!win.isDestroyed()) {
      try {
        win.webContents.send("kepler:commands:updated");
      } catch {}
    }
  }
}

ipcMain.handle("kepler:extension:install:do", async (_e, sourcePath: string) => {
  if (typeof sourcePath !== "string") {
    throw new Error("install:do: sourcePath must be a string");
  }
  const result = await installFromPath(sourcePath);
  notifyCommandsChanged();
  return result;
});

ipcMain.handle("kepler:extension:installed:list", async () => {
  return await listInstalledUserExtensions();
});

ipcMain.handle("kepler:extension:revert", async (_e, id: string, timestamp?: string) => {
  if (typeof id !== "string") {
    throw new Error("revert: id must be a string");
  }
  const result = await revertExtension(id, timestamp);
  notifyCommandsChanged();
  return result;
});

ipcMain.handle("kepler:extension:backups:list", async (_e, id: string) => {
  if (typeof id !== "string") {
    throw new Error("backups:list: id must be a string");
  }
  return await listBackups(id);
});

ipcMain.handle("kepler:extension:uninstall", async (_e, id: string) => {
  if (typeof id !== "string") {
    throw new Error("uninstall: id must be a string");
  }
  const result = await uninstallExtension(id);
  notifyCommandsChanged();
  return result;
});
