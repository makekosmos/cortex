const VAULT_PATH_PLACEHOLDER = "kepler://ark";
const SIDEBAR_STORAGE_KEY = "eden-extension-sidebar-config";
const VISIBLE_OBJECT_TYPE_IDS_STORAGE_KEY = "eden-extension-visible-object-type-ids";
const ZOOM_STORAGE_KEY = "eden-extension-zoom";

interface KeplerWindowApi {
  close: () => void;
  minimize: () => void;
  maximize: () => void;
  zoomGet?: () => Promise<number>;
  zoomSet?: (factor: number) => Promise<number>;
}

interface KeplerNamespace {
  window?: KeplerWindowApi;
}

interface SidebarConfig {
  widget: { width: number; hidden: boolean };
}

function keplerWindow(): KeplerWindowApi | null {
  const k = (window as unknown as { kepler?: KeplerNamespace }).kepler;
  return k?.window ?? null;
}

export async function getVaultPath(): Promise<string | null> {
  return VAULT_PATH_PLACEHOLDER;
}

export async function getRecentVaultPaths(): Promise<string[]> {
  return [VAULT_PATH_PLACEHOLDER];
}

export async function setVaultPath(_path: string): Promise<boolean> {
  return true;
}

export async function selectFolder(): Promise<string | null> {
  return null;
}

export async function openMarkdownVault(): Promise<MarkdownVaultOpenResult | null> {
  return window.kepler?.markdownFiles?.openVault?.() ?? null;
}

export async function saveMarkdownVault(
  files: MarkdownVaultExportFile[] = [],
): Promise<MarkdownVaultExportResult | null> {
  return window.kepler?.markdownFiles?.exportVault?.(files) ?? null;
}

export async function openMarkdownFile(): Promise<MarkdownFileOpenResult | null> {
  return window.kepler?.markdownFiles?.open?.() ?? null;
}

export async function saveMarkdownFile(
  suggestedName: string,
  content: string,
): Promise<MarkdownFileSaveResult | null> {
  return window.kepler?.markdownFiles?.save?.(suggestedName, content) ?? null;
}

export async function getSidebarConfig(): Promise<SidebarConfig> {
  return readSidebarConfig();
}

export async function updateSidebarConfig(patch: {
  widget?: { width?: number; hidden?: boolean };
}): Promise<SidebarConfig> {
  const current = readSidebarConfig();
  const next: SidebarConfig = {
    widget: {
      width: patch.widget?.width ?? current.widget.width,
      hidden: patch.widget?.hidden ?? current.widget.hidden,
    },
  };
  try {
    localStorage.setItem(SIDEBAR_STORAGE_KEY, JSON.stringify(next));
  } catch {
    // ignore storage failures
  }
  return next;
}

export async function getEdenVisibleObjectTypeIds(): Promise<string[]> {
  return readVisibleObjectTypeIds();
}

export async function setEdenVisibleObjectTypeIds(typeIds: string[]): Promise<string[]> {
  const seen = new Set<string>();
  const next = typeIds
    .filter((value): value is string => typeof value === "string")
    .map((value) => value.trim())
    .filter(Boolean)
    .filter((value) => {
      if (seen.has(value)) return false;
      seen.add(value);
      return true;
    });

  try {
    localStorage.setItem(VISIBLE_OBJECT_TYPE_IDS_STORAGE_KEY, JSON.stringify(next));
  } catch {
    // ignore storage failures
  }
  return next;
}

export async function getPlatform(): Promise<"win32" | "darwin" | "linux"> {
  const ua = navigator.userAgent.toLowerCase();
  if (ua.includes("windows")) return "win32";
  if (ua.includes("mac")) return "darwin";
  return "linux";
}

export async function zoomGet(): Promise<number> {
  const nativeZoomGet = keplerWindow()?.zoomGet;
  if (nativeZoomGet) {
    const nativeZoom = await nativeZoomGet().catch(() => null);
    if (nativeZoom !== null) return nativeZoom;
  }

  const raw = localStorage.getItem(ZOOM_STORAGE_KEY);
  const parsed = raw ? parseFloat(raw) : NaN;
  return Number.isFinite(parsed) ? parsed : 1;
}

export async function zoomSet(factor: number): Promise<number> {
  const clamped = Math.max(0.5, Math.min(2.0, factor));
  const nativeZoomSet = keplerWindow()?.zoomSet;

  if (nativeZoomSet) {
    const applied = await nativeZoomSet(clamped);
    try {
      localStorage.setItem(ZOOM_STORAGE_KEY, String(applied));
    } catch {
      // ignore
    }
    document.documentElement.style.zoom = "";
    return applied;
  }

  try {
    localStorage.setItem(ZOOM_STORAGE_KEY, String(clamped));
  } catch {
    // ignore
  }
  document.documentElement.style.zoom = String(clamped);
  return clamped;
}

export function minimize(): void {
  keplerWindow()?.minimize();
}

export function maximize(): void {
  keplerWindow()?.maximize();
}

export function close(): void {
  keplerWindow()?.close();
}

function readSidebarConfig(): SidebarConfig {
  try {
    const raw = localStorage.getItem(SIDEBAR_STORAGE_KEY);
    if (!raw) return { widget: { width: 320, hidden: false } };
    const parsed = JSON.parse(raw);
    return {
      widget: {
        width: Math.max(220, Math.min(520, Number(parsed?.widget?.width) || 320)),
        hidden: Boolean(parsed?.widget?.hidden ?? parsed?.widget?.collapsed ?? false),
      },
    };
  } catch {
    return { widget: { width: 320, hidden: false } };
  }
}

export function readVisibleObjectTypeIds(): string[] {
  try {
    const raw = localStorage.getItem(VISIBLE_OBJECT_TYPE_IDS_STORAGE_KEY);
    if (!raw) return [];
    const parsed = JSON.parse(raw);
    if (!Array.isArray(parsed)) return [];

    const seen = new Set<string>();
    return parsed
      .filter((value): value is string => typeof value === "string")
      .map((value) => value.trim())
      .filter(Boolean)
      .filter((value) => {
        if (seen.has(value)) return false;
        seen.add(value);
        return true;
      });
  } catch {
    return [];
  }
}
