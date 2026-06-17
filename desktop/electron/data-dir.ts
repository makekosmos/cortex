// Тонкая обёртка над instance.ts — оставлена для back-compat call site'ов,
// которые исторически звали `keplerDataDir()`. Резолюция (KOSMOS_DATA_DIR
// override, dev vs prod, slot suffix) живёт в `resolveInstance()`.

import { resolveInstance } from "./instance";

/**
 * Базовый dir под все Kepler-files: ark.db, kepler.lock.json, window
 * states, extensions/, extensions-data/, crashes/, backups/. НЕ хардкодь
 * «Kosmos» / «Kepler» нигде вне instance.ts.
 *
 *   prod:     %APPDATA%/Kosmos
 *   dev:      %APPDATA%/Kosmos-dev
 *   dev-<x>:  %APPDATA%/Kosmos-dev-<x>
 *   test:     значение KOSMOS_DATA_DIR (absolute, из Playwright helper'а)
 */
export function keplerDataDir(): string {
  return resolveInstance().dataDir;
}
