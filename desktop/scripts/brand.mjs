// Single source of truth for the Mundus product identity in desktop build
// scripts. Mirrors runtime/src/brand.rs — keep the two in sync.
//
// Persisted/legacy identifiers that must keep working are listed in
// docs/brand-legacy-identifiers.md.

export const PRODUCT_NAME = "Mundus";
export const PUBLISHER = "Kazui";

/** Installer file name: `Mundus-Setup-<ver>.exe`. */
export const installerName = (version) => `${PRODUCT_NAME}-Setup-${version}.exe`;

/** Per-user install dir: `%LOCALAPPDATA%\Programs\Mundus`. */
export const PROGRAMS_DIR_NAME = PRODUCT_NAME;
/** Roaming data root: `%APPDATA%\Mundus`. */
export const CONFIG_DIR_NAME = PRODUCT_NAME;
/** Local data root: `%LOCALAPPDATA%\Mundus` (Engine under `...\Mundus\Engine`). */
export const LOCAL_DIR_NAME = PRODUCT_NAME;

export const ENGINE_BINARY = "mundus-engine.exe";
export const ENGINE_ARCHIVE = "Mundus-Engine.zip";
export const ENGINE_ARCHIVE_STAGED = "Mundus Engine.zip";
export const ENGINE_MANIFEST_PRODUCT = "mundus-engine";
export const AUTOSTART_RUN_VALUE = "Mundus Engine";
export const UNINSTALL_KEY_NAME = "Mundus";

/** Packaged component executable names (staged under resources/components/*). */
export const COMPONENT_EXES = {
  manager: "Mundus Manager.exe",
  agenda: "Agenda.exe",
  memoria: "Memoria.exe",
  dictation: "Dictation.exe",
};

/** Read env var `MUNDUS_<suffix>` with legacy `KOSMOS_`/`KEPLER_` fallback. */
// MIGRATION(KOS-267): remove the legacy-prefix fallbacks after 2026-11-01.
export function env(suffix, env = process.env) {
  for (const prefix of ["MUNDUS_", "KOSMOS_", "KEPLER_"]) {
    const value = env[`${prefix}${suffix}`];
    if (value !== undefined && value !== "") return value;
  }
  return undefined;
}
