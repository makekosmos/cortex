import { app } from "electron";
import { existsSync, mkdirSync, readFileSync, readdirSync, statSync } from "node:fs";
import path from "node:path";
import net from "node:net";
import { fileURLToPath } from "node:url";
import { keplerDataDir } from "./data-dir";
import { KEPLER_API_VERSION, satisfiesSemver } from "./kepler-api";
import { loadRaycastPackageManifest, type RaycastPackageManifest } from "./raycast/manifest";
import type { ExtensionSource as ExtensionPermissionSource } from "./extension-permissions";

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
export interface ExtensionRootEntry {
  dir: string;
  source: ExtensionPermissionSource;
}

// Per-id lookup (`resolveExtensionLocation`) обходит цепочку и возвращает первый
// корень, где есть `manifest.json`. Это позволяет смешивать user-installed
// и bundled extensions.
export function resolveExtensionRootEntries(): ExtensionRootEntry[] {
  const roots: ExtensionRootEntry[] = [];
  // Repo dev tree: __dirname is platform/desktop/electron/ (or dist-electron/).
  // Source packages can live under products/*, incubator/*, or the deprecated
  // extensions/* compatibility root. In packaged builds we MUST skip this
  // branch entirely: otherwise `resources/extensions` (bundled first-party)
  // gets misclassified as dev and hides the prod/update flow.
  if (!app.isPackaged) {
    const repoRoot = path.resolve(__dirname, "..", "..", "..");
    for (const rootName of ["products", "incubator", "extensions"]) {
      const dev = path.join(repoRoot, rootName);
      if (existsSync(dev)) roots.push({ dir: dev, source: "dev" });
    }
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

export function resolveExtensionRoots(): string[] {
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

export function assertSafeUserDataName(name: unknown): asserts name is string {
  if (typeof name !== "string" || !USER_DATA_NAME_RE.test(name)) {
    throw new Error(`[kepler-shell] invalid user data file name: ${String(name)}`);
  }
}

export function resolveSafeUserDataPath(dir: string, name: unknown): string {
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

export function ensureUserDataDir(extId: string): string {
  const dir = extensionUserDataDir(extId);
  mkdirSync(dir, { recursive: true });
  return dir;
}

export function resolveExtensionLocation(
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

export function resolveExtensionDir(id: string): string | null {
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
  if (!manifest || !manifest.icon) return;
  const dir = resolveExtensionDir(id);
  if (!dir) return;
  const iconPath = path.resolve(path.join(dir, manifest.icon));
  if (!iconPath.startsWith(path.resolve(dir) + path.sep) && iconPath !== path.resolve(dir)) return;
  if (!existsSync(iconPath)) return;
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
    return;
  }
}

/**
 * Резолвит произвольный icon-path из манифеста (относительно extension dir)
 * в data:URI. Используется для `commands[].icon`. Возвращает undefined
 * если файл отсутствует / выходит за пределы extension dir / unreadable.
 */
function readManifestIconAsDataUri(id: string, iconRel: string): string | undefined {
  if (!iconRel || typeof iconRel !== "string") return;
  const dir = resolveExtensionDir(id);
  if (!dir) return;
  const resolved = path.resolve(path.join(dir, iconRel));
  // Path traversal guard: icon должна оставаться внутри extension dir.
  const dirResolved = path.resolve(dir);
  if (!resolved.startsWith(dirResolved + path.sep) && resolved !== dirResolved) return;
  if (!existsSync(resolved)) return;
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
    return;
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

export function resolveEntryHtml(manifest: ExtensionManifest, extensionDir: string): string {
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
