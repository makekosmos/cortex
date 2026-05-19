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
  ipcMain,
  screen,
  type WebContents,
} from "electron";
import {
  existsSync,
  mkdirSync,
  readFileSync,
  readdirSync,
  statSync,
  writeFileSync,
} from "node:fs";
import path from "node:path";
import { fileURLToPath } from "node:url";
import { keplerDataDir } from "./data-dir";
import { KEPLER_API_VERSION, satisfiesSemver } from "./kepler-api";
import {
  installFromPath,
  listInstalledUserExtensions,
  previewSource,
  revertExtension,
  listBackups,
  uninstallExtension,
} from "./extension-installer";

// ESM shim — __dirname / __filename не определены в Node ESM bundles.
const __filename = fileURLToPath(import.meta.url);
const __dirname = path.dirname(__filename);

export type ExtensionKind = "vue" | "static";

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
  mode?: "open" | "action";
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
   * Декларируемые permissions. Сейчас только документационно: показываются
   * в install dialog, но runtime не enforce'ит — extension получает полный
   * `window.kepler.*` API. Зарезервировано на будущее (capability model).
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
  entryHtml: string;
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
  /** Initial route переданный в `openExtension(id, route)` для cold start.
      Renderer читает через `kepler.navigation.initialRoute()` на mount. */
  initialRoute?: string;
}

const extensionWindows = new Map<string, ExtensionWindowEntry>();
// Reverse map: webContents.id → extension id. Нужен, чтобы из IPC handler'а
// определить, какое окно отправило запрос (kepler.window.close и т.п.).
const webContentsToExtensionId = new Map<number, string>();

// ArkClient injected lazily из main.ts через `setArkClient`. Если null —
// extension'ы получают ошибку при попытке ARK-запроса.
type ArkRequestFn = (req: Record<string, unknown>) => Promise<unknown>;
type ArkSubscribeFn = (
  event: string,
  handler: (payload: unknown) => void,
) => () => void;

let arkRequest: ArkRequestFn | null = null;
let arkSubscribe: ArkSubscribeFn | null = null;

// Ready-gate для extension ARK bridge. Extension windows могут открыться раньше,
// чем main.ts успеет вызвать `setExtensionArkBridge(...)` после handshake'а
// ArkClient'а. Если в этот момент extension probe'нет ARK (как Horologion делает
// в onMounted), он получит «ark bridge not ready» и UI запомнит status=error
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
      setTimeout(
        () => rej(new Error("ark bridge not ready (timeout)")),
        timeoutMs,
      ),
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
    const file = path.join(app.getPath("userData"), "kepler-shell-settings.json");
    if (!existsSync(file)) return false;
    const json = JSON.parse(readFileSync(file, "utf8")) as {
      developerMode?: boolean;
    };
    return !!json.developerMode;
  } catch {
    return false;
  }
}

function isDeveloperModeActive(): boolean {
  // KEPLER_DEV=1 включает только shell-уровень dev mode.
  // Extension dev mode (Vite HMR через manifest.devPort) требует явного
  // toggle в Settings — иначе extension'ы пытаются грузиться с dev server'а,
  // который может быть не запущен, и окно остаётся пустым.
  return readDevModeSetting();
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
// Per-id lookup (`resolveExtensionDir`) обходит цепочку и возвращает первый
// корень, где есть `manifest.json`. Это позволяет смешивать: Dashboard может
// быть user-installed, а Horologion — bundled.
function resolveExtensionRoots(): string[] {
  const roots: string[] = [];
  // Repo dev tree: __dirname is shell/electron/ (or shell/dist-electron/),
  // extensions are at <repoRoot>/extensions/ — i.e. ../../extensions/ from here.
  const dev = path.resolve(__dirname, "..", "..", "extensions");
  if (existsSync(dev)) roots.push(dev);
  const userRoot = path.join(keplerDataDir(), "extensions");
  if (!roots.includes(userRoot)) roots.push(userRoot);
  if (process.resourcesPath) {
    const bundled = path.join(process.resourcesPath, "extensions");
    if (!roots.includes(bundled)) roots.push(bundled);
  }
  return roots;
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

function assertSafeUserDataName(name: unknown): asserts name is string {
  if (typeof name !== "string" || !USER_DATA_NAME_RE.test(name)) {
    throw new Error(
      `[kepler-shell] invalid user data file name: ${String(name)}`,
    );
  }
}

function ensureUserDataDir(extId: string): string {
  const dir = extensionUserDataDir(extId);
  mkdirSync(dir, { recursive: true });
  return dir;
}

function resolveExtensionDir(id: string): string | null {
  for (const root of resolveExtensionRoots()) {
    const dir = path.join(root, id);
    if (existsSync(path.join(dir, "manifest.json"))) return dir;
  }
  return null;
}

function resolveSharedPreloadPath(): string {
  // Bundled by vite-plugin-electron alongside main.js: dist-electron/extension-preload.mjs
  return path.join(__dirname, "extension-preload.mjs");
}

export function loadExtensionManifest(id: string): ExtensionManifest | null {
  const dir = resolveExtensionDir(id);
  if (!dir) return null;
  const manifestPath = path.join(dir, "manifest.json");
  try {
    return JSON.parse(readFileSync(manifestPath, "utf8")) as ExtensionManifest;
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
  if (!iconPath.startsWith(path.resolve(dir) + path.sep) && iconPath !== path.resolve(dir)) return undefined;
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
      ext === ".svg" ? "image/svg+xml" : ext === ".jpg" || ext === ".jpeg" ? "image/jpeg" : "image/png";
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
      ext === ".svg" ? "image/svg+xml"
      : ext === ".jpg" || ext === ".jpeg" ? "image/jpeg"
      : ext === ".webp" ? "image/webp"
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
  /** Mode: `open` (openExtension) vs `action` (commands.invoke + auto-launch). */
  mode: "open" | "action";
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
        category: cmd.mode === "action" ? "action" : "open",
        kind: cmd.kind ?? "command",
        appName: manifest.name,
        icon,
        extensionId: manifest.id,
        route: cmd.route,
        mode: cmd.mode ?? "open",
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

function resolveEntryHtml(
  manifest: ExtensionManifest,
  extensionDir: string,
): string {
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
      padding: 6px 14px; border-radius: 6px; cursor: pointer; font: inherit; }
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
  const entry = extensionWindows.get(id);
  return !!entry && !entry.win.isDestroyed();
}

export function openExtension(id: string, route?: string): void {
  const existing = extensionWindows.get(id);
  if (existing && !existing.win.isDestroyed()) {
    if (process.env.KOSMOS_HEADLESS !== "1") existing.win.focus();
    if (route) {
      existing.win.webContents.send("kepler:extension:navigation", route);
    }
    return;
  }
  const manifest = loadExtensionManifest(id);
  if (!manifest) {
    console.warn(`[kepler-shell] extension not found: ${id}`);
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
  const extensionDir = resolveExtensionDir(id);
  if (!extensionDir) {
    console.warn(`[kepler-shell] extension dir disappeared: ${id}`);
    return;
  }
  const useDev = isDeveloperModeActive() && !!manifest.devPort;
  const entryHtml = resolveEntryHtml(manifest, extensionDir);
  if (!useDev && !existsSync(entryHtml)) {
    console.error(
      `[kepler-shell] extension '${id}' entryHtml not found: ${entryHtml}` +
        ` — для Vue extension'а сначала запусти build (bun run build:extensions).`,
    );
    return;
  }
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
      console.warn(
        `[kepler-shell] extension '${id}' window-state.json invalid, ignoring:`,
        e,
      );
    }
  }

  // Bounds sanitization: окно должно попадать хотя бы частично в какой-то
  // подключённый display. Иначе пользователь отключил монитор, на котором
  // окно стояло, и без проверки оно окажется за пределами экранов.
  const displays = screen.getAllDisplays();
  const isVisibleOnAnyDisplay = (
    x: number,
    y: number,
    w: number,
    h: number,
  ): boolean =>
    displays.some((d) => {
      const wa = d.workArea;
      return (
        x + w > wa.x &&
        x < wa.x + wa.width &&
        y + h > wa.y &&
        y < wa.y + wa.height
      );
    });

  let width = defaultWidth;
  let height = defaultHeight;
  let initialX: number | undefined =
    Math.round((display.width - defaultWidth) / 2);
  let initialY: number | undefined =
    Math.round((display.height - defaultHeight) / 2);

  if (
    typeof savedState.width === "number" &&
    typeof savedState.height === "number" &&
    typeof savedState.x === "number" &&
    typeof savedState.y === "number" &&
    isVisibleOnAnyDisplay(
      savedState.x,
      savedState.y,
      savedState.width,
      savedState.height,
    )
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

  // Windows backdrop material — opt-in через manifest.windowEffect.
  // Acrylic/mica требуют прозрачного backgroundColor; иначе native renderer
  // нарисует сплошной цвет поверх backdrop'а и эффект не будет виден.
  const wantsBackdrop =
    manifest.windowEffect === "acrylic" || manifest.windowEffect === "mica";
  const backgroundMaterial: "acrylic" | "mica" | undefined = wantsBackdrop
    ? (manifest.windowEffect as "acrylic" | "mica")
    : undefined;

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
    ...(backgroundMaterial ? { backgroundMaterial } : {}),
    // Native frame с hidden titlebar — custom controls рисует extension UI
    // (см. WindowControls в @kosmos/visuals, IPC kepler:extension:window:*).
    frame: true,
    titleBarStyle: "hidden",
    webPreferences: {
      preload,
      contextIsolation: true,
      nodeIntegration: false,
    },
  });

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
      writeFileSync(
        path.join(dir, "window-state.json"),
        JSON.stringify(state, null, 2),
        "utf8",
      );
    } catch (e) {
      console.warn(
        `[kepler-shell] extension '${id}' save window-state failed:`,
        e,
      );
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
      win.webContents.send(
        "kepler:extension:window:maximized-changed",
        win.isMaximized(),
      );
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
  win.on("close", () => {
    if (saveTimer) {
      clearTimeout(saveTimer);
      saveTimer = null;
    }
    saveWindowState();
  });

  // Capture webContents.id ДО регистрации listener'ов. После 'closed' event
  // BrowserWindow.webContents уже destroyed и обращение к нему кидает
  // "Object has been destroyed".
  const wcId = win.webContents.id;
  webContentsToExtensionId.set(wcId, id);
  win.on("closed", () => {
    webContentsToExtensionId.delete(wcId);
    extensionWindows.delete(id);
  });
  extensionWindows.set(id, { win, id, initialRoute: route });

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
    if (
      input.key === "F12" &&
      !input.alt &&
      !input.control &&
      !input.shift &&
      !input.meta
    ) {
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
  if (useDev && manifest.devPort) {
    const devUrl = `http://localhost:${manifest.devPort}/`;
    console.log(`[kepler-shell] extension '${id}' dev mode → ${devUrl}`);
    void win.loadURL(devUrl);
    win.webContents.openDevTools({ mode: "detach" });
  } else {
    void win.loadFile(entryHtml);
  }
}

function windowForSender(sender: WebContents): BrowserWindow | null {
  const win = BrowserWindow.fromWebContents(sender);
  return win && !win.isDestroyed() ? win : null;
}

function extensionIdForSender(sender: WebContents): string | null {
  return webContentsToExtensionId.get(sender.id) ?? null;
}

// ---------------------------------------------------------------------------
// IPC: list / open (kept from previous PoC contract)
// ---------------------------------------------------------------------------

ipcMain.handle("kepler:extension:list", () => listExtensions());
ipcMain.handle("kepler:extension:open", (_e, id: string) => openExtension(id));

// ---------------------------------------------------------------------------
// IPC: ARK proxy — extension renderer → main → ArkClient
// ---------------------------------------------------------------------------

ipcMain.handle(
  "kepler:extension:ark:request",
  async (_e, operation: string, params?: Record<string, unknown>) => {
    await awaitArkBridgeReady();
    if (!arkRequest) {
      throw new Error("ark bridge not ready");
    }
    const req: Record<string, unknown> = { operation, ...(params ?? {}) };
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

ipcMain.handle(
  "kepler:extension:ark:subscribe",
  async (e, event: string) => {
    await awaitArkBridgeReady();
    if (!arkSubscribe) {
      throw new Error("ark bridge not ready");
    }
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
  },
);

ipcMain.handle(
  "kepler:extension:ark:unsubscribe",
  (e, event: string) => {
    const key = `${e.sender.id}:${event}`;
    const unsubscribe = extensionEventUnsubscribers.get(key);
    if (unsubscribe) {
      unsubscribe();
      extensionEventUnsubscribers.delete(key);
    }
    return true;
  },
);

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

// Контроль возможности maximize. Eden использует это в zen mode: после
// setMaximizable(false) Windows native double-click-on-titlebar
// больше не разворачивает окно — наш Vue dblclick handler (dock-corner)
// отрабатывает без флика "maximize → unmaximize".
ipcMain.handle(
  "kepler:extension:window:set-maximizable",
  (e, value: boolean) => {
    const win = windowForSender(e.sender);
    if (!win || win.isDestroyed()) return;
    win.setMaximizable(Boolean(value));
  },
);

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
ipcMain.handle(
  "kepler:extension:invoke-host",
  (_e, action: string, _payload?: unknown) => {
    console.error(`[kepler-shell] extension invoke-host: ${action} (no handler)`);
    return false;
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
  const extId = extensionIdForSender(sender);
  if (!extId) {
    throw new Error("[kepler-shell] userData: sender is not an extension");
  }
  return ensureUserDataDir(extId);
}

ipcMain.handle("kepler:extension:userData:path", (e) => {
  return senderUserDataDir(e.sender);
});

ipcMain.handle(
  "kepler:extension:userData:readJson",
  (e, name: string): unknown => {
    assertSafeUserDataName(name);
    const dir = senderUserDataDir(e.sender);
    const filePath = path.join(dir, name);
    if (!existsSync(filePath)) return null;
    try {
      return JSON.parse(readFileSync(filePath, "utf8")) as unknown;
    } catch (err) {
      console.warn(
        `[kepler-shell] userData.readJson failed for ${filePath}:`,
        err,
      );
      return null;
    }
  },
);

ipcMain.handle(
  "kepler:extension:userData:writeJson",
  (e, name: string, value: unknown): void => {
    assertSafeUserDataName(name);
    const dir = senderUserDataDir(e.sender);
    const filePath = path.join(dir, name);
    writeFileSync(filePath, JSON.stringify(value, null, 2), "utf8");
  },
);

ipcMain.handle(
  "kepler:extension:userData:readFile",
  (e, name: string): string | null => {
    assertSafeUserDataName(name);
    const dir = senderUserDataDir(e.sender);
    const filePath = path.join(dir, name);
    if (!existsSync(filePath)) return null;
    try {
      return readFileSync(filePath, "utf8");
    } catch (err) {
      console.warn(
        `[kepler-shell] userData.readFile failed for ${filePath}:`,
        err,
      );
      return null;
    }
  },
);

ipcMain.handle(
  "kepler:extension:userData:writeFile",
  (e, name: string, content: string): void => {
    assertSafeUserDataName(name);
    if (typeof content !== "string") {
      throw new Error("[kepler-shell] userData.writeFile: content must be a string");
    }
    const dir = senderUserDataDir(e.sender);
    const filePath = path.join(dir, name);
    writeFileSync(filePath, content, "utf8");
  },
);

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

ipcMain.handle("kepler:extension:install:preview", (_e, sourcePath: string) => {
  if (typeof sourcePath !== "string") {
    throw new Error("install:preview: sourcePath must be a string");
  }
  return previewSource(sourcePath);
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
      try { win.webContents.send("kepler:commands:updated"); } catch {}
    }
  }
}

ipcMain.handle("kepler:extension:install:do", (_e, sourcePath: string) => {
  if (typeof sourcePath !== "string") {
    throw new Error("install:do: sourcePath must be a string");
  }
  const result = installFromPath(sourcePath);
  notifyCommandsChanged();
  return result;
});

ipcMain.handle("kepler:extension:installed:list", () =>
  listInstalledUserExtensions(),
);

ipcMain.handle("kepler:extension:revert", (_e, id: string, timestamp?: string) => {
  if (typeof id !== "string") {
    throw new Error("revert: id must be a string");
  }
  const result = revertExtension(id, timestamp);
  notifyCommandsChanged();
  return result;
});

ipcMain.handle("kepler:extension:backups:list", (_e, id: string) => {
  if (typeof id !== "string") {
    throw new Error("backups:list: id must be a string");
  }
  return listBackups(id);
});

ipcMain.handle("kepler:extension:uninstall", (_e, id: string) => {
  if (typeof id !== "string") {
    throw new Error("uninstall: id must be a string");
  }
  const result = uninstallExtension(id);
  notifyCommandsChanged();
  return result;
});
