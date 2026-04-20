import { shallowRef } from "vue";

export interface SidebarConfig {
  width: number;
  hidden: boolean;
}

export const SIDEBAR_STORAGE_KEY = "arrancador-sidebar-config";
export const SIDEBAR_DEFAULT_WIDTH = 200;
export const SIDEBAR_MIN_WIDTH = 160;
export const SIDEBAR_MAX_WIDTH = 320;

function sanitizeSidebarWidth(width?: number): number {
  if (typeof width !== "number" || !Number.isFinite(width)) {
    return SIDEBAR_DEFAULT_WIDTH;
  }

  return Math.min(SIDEBAR_MAX_WIDTH, Math.max(SIDEBAR_MIN_WIDTH, width));
}

export function sanitizeSidebarConfig(
  config?: Partial<SidebarConfig>,
): SidebarConfig {
  return {
    width: sanitizeSidebarWidth(config?.width),
    hidden: config?.hidden ?? false,
  };
}

function loadStoredSidebarConfig() {
  if (typeof window === "undefined") {
    return sanitizeSidebarConfig();
  }

  try {
    const raw = window.localStorage.getItem(SIDEBAR_STORAGE_KEY);
    if (!raw) {
      return sanitizeSidebarConfig();
    }

    return sanitizeSidebarConfig(JSON.parse(raw) as Partial<SidebarConfig>);
  } catch {
    return sanitizeSidebarConfig();
  }
}

const sidebarConfig = shallowRef<SidebarConfig>(sanitizeSidebarConfig());

export function initializeSidebarConfig() {
  sidebarConfig.value = loadStoredSidebarConfig();
}

export function useSidebarConfig() {
  const persistSidebarConfig = (nextConfig: SidebarConfig) => {
    const sanitized = sanitizeSidebarConfig(nextConfig);
    sidebarConfig.value = sanitized;

    if (typeof window !== "undefined") {
      try {
        window.localStorage.setItem(
          SIDEBAR_STORAGE_KEY,
          JSON.stringify(sanitized),
        );
      } catch {
        // Ignore persistence failures.
      }
    }
  };

  return {
    sidebarConfig,
    persistSidebarConfig,
  };
}
