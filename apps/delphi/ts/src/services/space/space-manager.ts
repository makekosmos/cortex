/**
 * Delphi Space Manager — re-exports generic space functions from @arksync/core
 * and adds Delphi-specific localStorage persistence.
 */

import { formatSpaceCode as _formatSpaceCode } from "@arksync/core";

export {
  generateSpaceCode, encodeIpv4, decodeIpv4, generateExtendedCode,
  formatSpaceCode, parseSpaceCode, generateQrPayload, parseQrPayload,
  deriveSpaceId,
} from "@arksync/core";

// ---------------------------------------------------------------------------
// Delphi-specific: Space persistence (localStorage)
// ---------------------------------------------------------------------------

const SPACES_KEY = "delphi.spaces";
const ACTIVE_SPACE_KEY = "delphi.active_space";

export interface Space {
  code: string;
  name: string;
  createdAt: string;
}

export function getSpaces(): Space[] {
  try {
    return JSON.parse(localStorage.getItem(SPACES_KEY) || "[]");
  } catch {
    return [];
  }
}

export function saveSpace(space: Space): void {
  const spaces = getSpaces().filter((s) => s.code !== space.code);
  spaces.unshift(space);
  localStorage.setItem(SPACES_KEY, JSON.stringify(spaces));
}

export function removeSpace(code: string): void {
  const spaces = getSpaces().filter((s) => s.code !== code);
  localStorage.setItem(SPACES_KEY, JSON.stringify(spaces));
}

export function renameSpace(code: string, newName: string): boolean {
  const spaces = getSpaces();
  const space = spaces.find((s) => s.code === code);
  if (!space) return false;
  space.name = newName.trim() || _formatSpaceCode(code);
  localStorage.setItem(SPACES_KEY, JSON.stringify(spaces));
  return true;
}

export function getActiveSpace(): string | null {
  return localStorage.getItem(ACTIVE_SPACE_KEY) || null;
}

export function setActiveSpace(code: string | null): void {
  if (code) {
    localStorage.setItem(ACTIVE_SPACE_KEY, code);
  } else {
    localStorage.removeItem(ACTIVE_SPACE_KEY);
  }
}
