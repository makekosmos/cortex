// Single source of truth для базового data dir.
//
// Production install: %APPDATA%\Kosmos\
// Dev mode (`bun run --cwd shell dev`, либо VITE_DEV_SERVER_URL set):
//   %APPDATA%\Kosmos-dev\
// Test (Playwright e2e, KOSMOS_DATA_DIR env): абсолютный путь под
//   tests/.e2e/<slug>/
//
// Backend (services/kepler-backend) тоже читает KOSMOS_DATA_DIR — shell
// передаёт его в env при spawn'е backend'а, чтобы shell+backend смотрели
// на один и тот же ark.db. dev script (shell/package.json) выставляет
// KOSMOS_DATA_DIR=%APPDATA%/Kosmos-dev до запуска vite, и main.ts здесь
// падает в production path только когда install setup'нул нас.

import path from "node:path";
import { app } from "electron";

/**
 * Базовый dir под все Kepler-files: ark.db, kepler.lock.json, window
 * states, extensions/, extensions-data/. Каждый caller строит свой
 * sub-path сверху. НЕ хардкодь «Kosmos» нигде вне этого модуля.
 */
export function keplerDataDir(): string {
  const override = process.env.KOSMOS_DATA_DIR;
  if (override && override.length > 0) {
    return override;
  }
  const isDev = !!process.env.VITE_DEV_SERVER_URL;
  const dirName = isDev ? "Kosmos-dev" : "Kosmos";
  return path.join(app.getPath("appData"), dirName);
}
