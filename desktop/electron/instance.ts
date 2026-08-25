// Instance slot resolution — single source of truth для разделения
// prod / dev / multi-dev / test инстансов Kepler.
//
// Проблема: до этого модуля dev (`bun run --cwd platform/desktop dev`) и prod
// (installed Kepler.exe) шарили `app.requestSingleInstanceLock()` (через
// `appId=com.kazui.kepler`) и Electron `userData` (через `productName=Kepler`).
// Параллельный запуск был невозможен — первый брал lock, второй редиректил
// focus и сразу выходил.
//
// Решение: каждой сборке/worktree выдаём `slot` (`prod` | `dev` | `dev-<x>` |
// `test-<x>`). На его основе derive'им userData, ARK dataDir, productName,
// hotkey, autoupdater on/off, autorun allowed/forbidden. SingleInstanceLock
// и Electron paths scope'ятся по userData, hotkey'и не конфликтуют.
//
// Resolution priority:
//   1. KEPLER_INSTANCE env (explicit)
//   2. KOSMOS_DATA_DIR set → test-<basename(dir)>
//   3. VITE_DEV_SERVER_URL set → "dev"
//   4. иначе → "prod"
//
// Multi-agent worktree: каждый worktree кладёт `platform/desktop/.env.local` с
// `KEPLER_INSTANCE=dev-<slug>` (см. .env.local.example). dev.mjs читает
// .env.local и пробрасывает env.
//
// ВАЖНО: resolveInstance() должен быть вызван ДО любого app.setName /
// app.setPath / app.requestSingleInstanceLock — иначе lock уже взят под
// дефолтным именем "Kepler" и разделения не будет.

import { app } from "electron";
import path from "node:path";
import {
  copyFileSync,
  cpSync,
  existsSync,
  mkdirSync,
  readFileSync,
  readdirSync,
  writeFileSync,
} from "node:fs";

type InstanceKind = "prod" | "dev" | "test";

export interface Instance {
  /** Канонический id слота: "prod" | "dev" | "dev-<slug>" | "test-<slug>". */
  slot: string;
  kind: InstanceKind;
  /** Electron userData (singleInstanceLock scope, settings, window state). */
  userDataDir: string;
  /** ARK data dir (ark.db, kepler.lock.json, extensions/, crashes/, backups/). */
  dataDir: string;
  /** Отображаемое имя приложения (tray tooltip, dialog title). */
  productName: string;
  /** Application User Model ID (Windows taskbar grouping). */
  appId: string;
  /** GlobalShortcut accelerator. null = hotkey disabled для этого слота. */
  hotkey: string | null;
  /** Autoupdater (electron-updater) разрешён? Только в prod. */
  autoupdaterEnabled: boolean;
  /** HKCU Run / login item можно трогать? Только в prod. */
  autorunEnabled: boolean;
  /** Периодический marketplace catalog fetch разрешён? Только в prod. */
}

interface UserDataMatch {
  ok: boolean;
  expected: string;
  actual: string;
}

const SLOT_RE = /^(prod|dev|dev-[a-z0-9][a-z0-9-]*|test-[a-z0-9][a-z0-9-]*)$/;

let cached: Instance | null = null;

function env(name: string): string | undefined {
  return process.env[name];
}

function instanceKindForSlot(slot: string): InstanceKind {
  if (slot.startsWith("test")) return "test";
  if (slot === "prod") return "prod";
  return "dev";
}

function productNameForSlot(slot: string, kind: InstanceKind): string {
  if (slot === "prod") return "Kosmos";
  if (slot === "dev") return "Kosmos [dev]";
  if (kind === "test") return "Kosmos [test]";
  return `Kosmos [${slot}]`;
}

function hotkeyForSlot(slot: string): string | null {
  if (slot === "prod") {
    return process.platform === "darwin" ? "Command+Space" : "Alt+Space";
  }
  if (slot === "dev") return "Alt+`";
  return null;
}

/**
 * Резолвит instance slot и derive'нутые пути / флаги. Идемпотентно —
 * результат кэшируется (env читается один раз на module load).
 */
export function resolveInstance(): Instance {
  if (cached) return cached;
  const slot = pickSlot();
  if (!SLOT_RE.test(slot)) {
    throw new Error(
      `[kepler-shell] invalid KEPLER_INSTANCE="${slot}" — ожидался prod | dev | dev-<slug> | test-<slug>`,
    );
  }

  const appData = app.getPath("appData");
  const kind = instanceKindForSlot(slot);

  // ARK dataDir:
  //   - test-<x>: KOSMOS_DATA_DIR env (всегда absolute). Helper'ы Playwright
  //     выставляют под tests/.e2e/<spec>/. Никогда не fallback'аем в %APPDATA%.
  //   - prod: %APPDATA%/Kosmos
  //   - dev: %APPDATA%/Kosmos-dev
  //   - dev-<x>: %APPDATA%/Kosmos-dev-<x>
  let dataDir: string;
  const dataDirOverride = env("KOSMOS_DATA_DIR");
  if (kind === "test") {
    if (!dataDirOverride) {
      throw new Error(`[kepler-shell] test slot "${slot}" требует KOSMOS_DATA_DIR env`);
    }
    dataDir = dataDirOverride;
  } else if (slot === "prod") {
    dataDir = dataDirOverride || path.join(appData, "Kosmos");
  } else if (slot === "dev") {
    dataDir = dataDirOverride || path.join(appData, "Kosmos-dev");
  } else {
    // dev-<x>
    const suffix = slot.slice("dev-".length);
    dataDir = dataDirOverride || path.join(appData, `Kosmos-dev-${suffix}`);
  }

  // Electron userData (singleInstanceLock scope, kepler-shell-settings.json,
  // post-update.flag, window state cache, GPU cache, Local Storage).
  //   - prod: %APPDATA%/Kosmos App (display product rename; ARK data remains
  //     %APPDATA%/Kosmos)
  //   - dev: %APPDATA%/Kosmos App-dev
  //   - dev-<x>: %APPDATA%/Kosmos App-dev-<x>
  //   - test-<x>: <KOSMOS_DATA_DIR>/userdata (под data dir, чтобы Playwright
  //     auto-cleanup tests/.e2e/<slug>/ снёс и userData тоже)
  let userDataDir: string;
  if (kind === "test") {
    userDataDir = path.join(dataDir, "userdata");
  } else if (slot === "prod") {
    userDataDir = path.join(appData, "Kosmos App");
  } else if (slot === "dev") {
    userDataDir = path.join(appData, "Kosmos App-dev");
  } else {
    const suffix = slot.slice("dev-".length);
    userDataDir = path.join(appData, `Kosmos App-dev-${suffix}`);
  }

  const productName = productNameForSlot(slot, kind);

  const appId = slot === "prod" ? "com.kazui.kosmos" : `com.kazui.kosmos.${slot}`;

  // Hotkey: prod использует platform-native accelerator; dev = Alt+`
  // (legacy, не конфликтует с prod-инстансом). dev-<x> и test-<x> =
  // disabled (несколько dev-инстансов не могут поделить один accelerator;
  // пользователь активирует launcher через tray click).
  const hotkey = hotkeyForSlot(slot);

  return (cached = {
    slot,
    kind,
    userDataDir,
    dataDir,
    productName,
    appId,
    hotkey,
    autoupdaterEnabled: kind === "prod",
    autorunEnabled: kind === "prod",
  });
}

function pickSlot(): string {
  const fromEnv = (env("KEPLER_INSTANCE") || "").trim();
  if (fromEnv) return fromEnv;

  const kosmosDataDir = env("KOSMOS_DATA_DIR");
  if (kosmosDataDir) {
    const base = path
      .basename(kosmosDataDir)
      .toLowerCase()
      .replace(/[^a-z0-9-]/g, "-");
    return `test-${base || "default"}`;
  }

  if (env("VITE_DEV_SERVER_URL")) return "dev";

  return "prod";
}

/**
 * Применяет резолвнутый instance к Electron app до того как Electron
 * сделает первое чтение `app.getName()` / `app.getPath('userData')` —
 * единственное место где это допустимо.
 *
 * MUST быть вызвано ДО `app.requestSingleInstanceLock()`. Вызывать ровно
 * один раз на старте main.ts, в самом верху.
 *
 * Side effect: one-shot миграция settings.json из старого `%APPDATA%/Kepler/`
 * в новый dev userData, если slot=dev и dev userData пуст. Это smoothes
 * upgrade'у — dev пользователь не теряет developerMode toggle и hotkey
 * override после первого запуска с новой системой.
 */
export function applyInstanceToApp(instance: Instance): void {
  if (instance.slot === "prod") {
    migrateLegacyProdUserData(instance.userDataDir);
    migrateLegacyProdSettings(app.getPath("appData"), instance.dataDir);
  }
  app.setName(instance.productName);
  app.setPath("userData", instance.userDataDir);
  // Windows taskbar grouping: без setAppUserModelId два инстанса с одним
  // appId могут сгруппироваться под одну иконку в taskbar.
  if (process.platform === "win32") {
    try {
      app.setAppUserModelId(instance.appId);
    } catch {
      /* ignore — не критично */
    }
  }

  if (instance.slot === "dev") {
    migrateLegacyDevSettings(instance.userDataDir);
  }
}

function isMissingOrEmptyDir(dir: string): boolean {
  if (!existsSync(dir)) return true;
  try {
    return readdirSync(dir).length === 0;
  } catch {
    return false;
  }
}

/**
 * Kepler → Kosmos product rename migration.
 *
 * ARK data already lives in `%APPDATA%/Kosmos` and must not move. This copies
 * only Electron userData (`Local Storage`, GPU cache, update flags, etc.) from
 * the old product folder so existing installs keep their UI state.
 */
function migrateLegacyProdUserData(newUserDataDir: string): void {
  try {
    if (!isMissingOrEmptyDir(newUserDataDir)) return;
    const legacy = path.join(app.getPath("appData"), "Kepler");
    if (!existsSync(legacy)) return;
    mkdirSync(newUserDataDir, { recursive: true });
    cpSync(legacy, newUserDataDir, {
      recursive: true,
      force: false,
      errorOnExist: false,
    });
    console.error(`[kosmos] migrated Electron userData: %APPDATA%/Kepler -> ${newUserDataDir}`);
  } catch (e) {
    console.error("[kosmos] prod userData migration skipped:", e);
  }
}

export function migrateLegacyProdSettings(appData: string, dataDir: string): void {
  try {
    const target = path.join(dataDir, "kepler-shell-settings.json");
    if (existsSync(target)) return;
    const legacy = path.join(appData, "Kepler", "kepler-shell-settings.json");
    if (!existsSync(legacy)) return;
    mkdirSync(dataDir, { recursive: true });
    copyFileSync(legacy, target);
    console.error(`[kosmos] migrated shell settings: ${legacy} -> ${target}`);
  } catch (e) {
    console.error("[kosmos] shell settings migration skipped:", e);
  }
}

/**
 * Phase 7 boot self-check helper. Возвращает { ok: true } если Electron
 * закэшировал тот же `userData` который мы насчитали в `resolveInstance()`,
 * иначе { ok: false, expected, actual }. Caller (main.ts::runBootSelfCheck)
 * показывает error dialog и exit'ит. Эта функция инкапсулирует единственный
 * легитимный вызов `app.getPath('userData')` вне `applyInstanceToApp` —
 * guard `ark:guard:writes` whitelist'ит только этот файл.
 */
export function verifyUserDataMatches(instance: Instance): UserDataMatch {
  const actual = app.getPath("userData");
  return { ok: actual === instance.userDataDir, expected: instance.userDataDir, actual };
}

/**
 * До этого рефакторинга dev писал в `%APPDATA%/Kepler/` (тот же что prod).
 * После — `%APPDATA%/Kepler-dev/`. Чтобы dev пользователь не потерял
 * developerMode toggle, hotkey override и т.п., копируем kepler-shell-settings.json
 * один раз при первом запуске нового dev userData.
 *
 * Не трогаем: post-update.flag (autoupdater state не релевантен для dev),
 * Local Storage / Cache (Chromium регенерирует), window state (под dataDir,
 * не userData).
 */
function migrateLegacyDevSettings(newUserDataDir: string): void {
  try {
    const newSettings = path.join(newUserDataDir, "kepler-shell-settings.json");
    if (existsSync(newSettings)) return; // уже мигрировано или dev user уже что-то писал
    const candidates = [
      path.join(app.getPath("appData"), "Kosmos App", "kepler-shell-settings.json"),
      path.join(app.getPath("appData"), "Kepler-dev", "kepler-shell-settings.json"),
      path.join(app.getPath("appData"), "Kepler", "kepler-shell-settings.json"),
    ];
    const legacy = candidates.find((p) => existsSync(p));
    if (!legacy) return; // нет источника — first-time dev user
    const content = readFileSync(legacy, "utf8");
    mkdirSync(newUserDataDir, { recursive: true });
    writeFileSync(newSettings, content, "utf8");
    console.error(`[kosmos] migrated dev settings: ${legacy} -> ${newUserDataDir}`);
  } catch (e) {
    // Не критично — dev user заново выставит developer mode toggle.
    console.error("[kepler-shell] dev settings migration skipped:", e);
  }
}
