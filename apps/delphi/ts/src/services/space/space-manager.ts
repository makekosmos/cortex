/**
 * Delphi Space Manager — re-exports generic space functions from @arksync/core
 * and adds Delphi-specific persistence.
 *
 * On Electron: persists via IPC to main process (JSON file in appData — survives localStorage wipe).
 * On Web: falls back to localStorage.
 */

import { formatSpaceCode as _formatSpaceCode } from "@arksync/core";

export {
  generateSpaceCode,
  encodeIpv4,
  decodeIpv4,
  generateExtendedCode,
  formatSpaceCode,
  parseSpaceCode,
  generateQrPayload,
  parseQrPayload,
  deriveSpaceId,
} from "@arksync/core";

// ---------------------------------------------------------------------------
// Types
// ---------------------------------------------------------------------------

export interface Space {
  code: string;
  name: string;
  createdAt: string;
}

// ---------------------------------------------------------------------------
// Electron IPC persistence (primary — file-backed, survives localStorage wipe)
// ---------------------------------------------------------------------------

const isElectron = !!(window as unknown as Record<string, unknown>).electronAPI;

function ipc(): {
  invoke: (channel: string, ...args: unknown[]) => Promise<unknown>;
} | null {
  return (window as unknown as Record<string, unknown>)
    .electronAPI as ReturnType<typeof ipc>;
}

// ---------------------------------------------------------------------------
// Public API
// ---------------------------------------------------------------------------

export async function getSpaces(): Promise<Space[]> {
  if (isElectron) {
    try {
      const result = await ipc()?.invoke("space:getAll");
      if (Array.isArray(result)) return result as Space[];
    } catch {}
  }
  // Fallback: localStorage
  try {
    return JSON.parse(localStorage.getItem("delphi.spaces") || "[]");
  } catch {
    return [];
  }
}

export async function saveSpace(space: Space): Promise<void> {
  if (isElectron) {
    try {
      await ipc()?.invoke("space:save", space);
    } catch {}
  }
  // Also keep localStorage as fallback
  const spaces = await getSpaces();
  const filtered = spaces.filter((s) => s.code !== space.code);
  filtered.unshift(space);
  localStorage.setItem("delphi.spaces", JSON.stringify(filtered));
}

export async function removeSpace(code: string): Promise<void> {
  if (isElectron) {
    try {
      await ipc()?.invoke("space:remove", code);
    } catch {}
  }
  const spaces = await getSpaces();
  localStorage.setItem(
    "delphi.spaces",
    JSON.stringify(spaces.filter((s) => s.code !== code)),
  );
}

export async function renameSpace(
  code: string,
  newName: string,
): Promise<boolean> {
  if (isElectron) {
    try {
      const result = await ipc()?.invoke("space:rename", code, newName);
      if (result) {
        // Sync localStorage
        const spaces = await getSpaces();
        const space = spaces.find((s) => s.code === code);
        if (space) {
          space.name = newName.trim() || _formatSpaceCode(code);
          localStorage.setItem("delphi.spaces", JSON.stringify(spaces));
        }
        return true;
      }
    } catch {}
  }
  // Fallback
  try {
    const spaces: Space[] = JSON.parse(
      localStorage.getItem("delphi.spaces") || "[]",
    );
    const space = spaces.find((s) => s.code === code);
    if (!space) return false;
    space.name = newName.trim() || _formatSpaceCode(code);
    localStorage.setItem("delphi.spaces", JSON.stringify(spaces));
    return true;
  } catch {
    return false;
  }
}

export async function getActiveSpace(): Promise<string | null> {
  if (isElectron) {
    try {
      const result = await ipc()?.invoke("space:getActive");
      if (typeof result === "string") return result;
    } catch {}
  }
  return localStorage.getItem("delphi.active_space") || null;
}

export async function setActiveSpace(code: string | null): Promise<void> {
  if (isElectron) {
    try {
      await ipc()?.invoke("space:setActive", code);
    } catch {}
  }
  if (code) {
    localStorage.setItem("delphi.active_space", code);
  } else {
    localStorage.removeItem("delphi.active_space");
  }
}

/** Get the path to the databases directory (Electron only). */
export async function getDbPath(): Promise<string | null> {
  if (isElectron) {
    try {
      const result = await ipc()?.invoke("space:getDbPath");
      if (typeof result === "string") return result;
    } catch {}
  }
  return null;
}
