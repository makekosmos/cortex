// Helper для Kepler shell Electron launch в e2e тестах.
//
// Жёсткое правило (см. docs-site/concepts/test-isolation.md):
//   - НИКОГДА не указывать KOSMOS_DATA_DIR на user dir (%APPDATA%\Kosmos).
//   - Каждый тест получает свежий dir под `tests/.e2e/<spec>/<test>/`,
//     pre-cleaned before тестом.
//   - Backend читает KOSMOS_DATA_DIR (см. kepler-backend/src/lock_file.rs);
//     lock-файл, singleton.lock, default ark.db — всё под этим dir.

import path from "node:path";
import fs from "node:fs";
import { fileURLToPath } from "node:url";
import { _electron as electron, type ElectronApplication } from "playwright";
import electronBinary from "electron";

const __filename = fileURLToPath(import.meta.url);
const __dirname = path.dirname(__filename);

export const REPO_ROOT = path.resolve(__dirname, "..", "..", "..");
const SHELL_ROOT = path.join(REPO_ROOT, "platform", "desktop");
const E2E_ROOT = path.join(REPO_ROOT, "tests", ".e2e");

export interface LaunchOptions {
  /** Имя tмп-dir под `tests/.e2e/<slug>/`. Pre-cleaned. */
  slug: string;
  /** Доп. env поверх process.env. KOSMOS_DATA_DIR форсится автоматически. */
  env?: Record<string, string>;
}

export function freshDataDir(slug: string): string {
  const dir = path.join(E2E_ROOT, slug);
  // Sanity: убеждаемся что dir НЕ ведёт в user data dir.
  const appData = process.env.APPDATA ?? "";
  if (appData && dir.toLowerCase().startsWith(appData.toLowerCase())) {
    throw new Error(
      `freshDataDir refuses: target ${dir} находится внутри %APPDATA% (${appData}). ` +
        `Это нарушает test isolation policy.`,
    );
  }
  fs.rmSync(dir, { recursive: true, force: true });
  fs.mkdirSync(dir, { recursive: true });
  return dir;
}

async function _launch(
  dataDir: string,
  extraEnv?: Record<string, string>,
): Promise<ElectronApplication> {
  const userDataDir = path.join(dataDir, "electron-userdata");
  fs.mkdirSync(userDataDir, { recursive: true });

  const mainJs = path.join(SHELL_ROOT, "dist-electron", "main.js");
  if (!fs.existsSync(mainJs)) {
    throw new Error(
      `${mainJs} не существует. Сначала запусти 'bun run --cwd platform/desktop build:js'.`,
    );
  }

  return electron.launch({
    executablePath: electronBinary,
    cwd: SHELL_ROOT,
    args: [mainJs, `--user-data-dir=${userDataDir}`],
    env: {
      ...process.env,
      NODE_ENV: "test",
      KOSMOS_DATA_DIR: dataDir,
      KOSMOS_TEST_MODE: "1",
      // Headless mode: extension windows создаются с show:false + skipTaskbar.
      // Playwright всё равно может evaluate() и locator() работать через
      // webContents без visible render. См. platform/desktop/electron/extension-host.ts.
      KOSMOS_HEADLESS: "1",
      // Test-only: backend пропускает icacls/chmod hardening на kepler.lock.json.
      // Без этого stale lock-файл от прошлого Windows account'а блокирует
      // freshDataDir с EPERM. См. docs-site/agents/testing.md → "Stale ACL".
      KOSMOS_LOCK_PERMISSIONS_DISABLED: "1",
      KEPLER_SKIP_SYNC: "1",
      KEPLER_USAGE_TRACKER: "0",
      ...extraEnv,
    },
    timeout: 20_000,
  });
}

/**
 * Запускает Kepler с УЖЕ существующим (заранее подготовленным) dataDir —
 * не вызывает freshDataDir (который бы снёс заготовку).
 *
 * Используется в post-update spec'ах: тесту нужно записать
 * `<dataDir>/userdata/post-update.flag` ДО старта main process'а.
 */
export async function launchKeplerWithDataDir(
  dataDir: string,
  extraEnv?: Record<string, string>,
): Promise<ElectronApplication> {
  return _launch(dataDir, extraEnv);
}

export async function launchKepler(opts: LaunchOptions): Promise<ElectronApplication> {
  const dataDir = freshDataDir(opts.slug);
  return _launch(dataDir, opts.env);
}
