// Pure (electron-free) helpers describing how an extension window should be
// chromed depending on its "profile". Kept out of extension-host.ts so the
// geometry/trait logic is unit-testable without an Electron runtime.
//
//   - "default": normal extension window — restores/persists per-id geometry,
//     maximizable, manifest-driven backdrop.
//   - "settings": Kepler-settings-style window — compact, centered, fixed size,
//     non-maximizable, acrylic, and NEVER persists geometry (so it can't clobber
//     the host extension's own window-state.json).

export type ExtensionWindowProfile = "default" | "settings";

/** Dedicated settings-window size — mirrors the Kepler shell settings window
 *  (`settings-window.ts` SETTINGS_WIDTH/HEIGHT) so the silhouette matches. */
export const SETTINGS_WINDOW_SIZE = {
  width: 880,
  height: 560,
  minWidth: 800,
  minHeight: 560,
} as const;

export interface SettingsWindowBounds {
  width: number;
  height: number;
  minWidth: number;
  minHeight: number;
  x: number;
  y: number;
}

/** Centered, fixed-size bounds for the settings window on a given work area. */
export function settingsWindowBounds(workArea: {
  width: number;
  height: number;
}): SettingsWindowBounds {
  const { width, height, minWidth, minHeight } = SETTINGS_WINDOW_SIZE;
  return {
    width,
    height,
    minWidth,
    minHeight,
    x: Math.round((workArea.width - width) / 2),
    y: Math.round((workArea.height - height) / 2),
  };
}

export interface ExtensionWindowProfileTraits {
  /** Restore & persist `extensions-data/<id>/window-state.json`. */
  persistWindowState: boolean;
  maximizable: boolean;
  fullscreenable: boolean;
  /** Force the acrylic backdrop regardless of the manifest's windowEffect. */
  forceAcrylic: boolean;
}

export function windowProfileTraits(profile: ExtensionWindowProfile): ExtensionWindowProfileTraits {
  if (profile === "settings") {
    return {
      persistWindowState: false,
      maximizable: false,
      fullscreenable: false,
      forceAcrylic: true,
    };
  }
  return {
    persistWindowState: true,
    maximizable: true,
    fullscreenable: true,
    forceAcrylic: false,
  };
}
