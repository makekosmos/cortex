/**
 * Ark Space manager -- generates, stores, and retrieves space codes.
 *
 * v3: 12-character random Base32-Crockford code, format XXXX-XXXX-XXXX.
 * All peers are equal -- the code is used as HMAC secret for auth.
 *
 * QR payload: ark://join?code=XXXX-XXXX-XXXX&addrs=addr1,addr2
 */

const CROCKFORD = "0123456789ABCDEFGHJKMNPQRSTVWXYZ"; // no I, L, O, U
const SPACES_KEY = "delphi.spaces";
const ACTIVE_SPACE_KEY = "delphi.active_space";

export interface Space {
  code: string; // raw code (no dashes), 12 chars
  name: string; // human-readable label
  createdAt: string; // ISO 8601
}

/** Generate a random 12-character Base32-Crockford code. */
export function generateSpaceCode(): string {
  const arr = new Uint8Array(12);
  crypto.getRandomValues(arr);
  return Array.from(arr, (b) => CROCKFORD[b % 32]).join("");
}

/**
 * Format code with dashes -- handles both 7-char (XXXX-XXX, legacy) and
 * 12-char (XXXX-XXXX-XXXX) codes.
 */
export function formatSpaceCode(code: string): string {
  const clean = code.replace(/[-\s]/g, "").toUpperCase();
  if (clean.length === 7) {
    return `${clean.slice(0, 4)}-${clean.slice(4, 7)}`;
  }
  // 12-char (standard)
  if (clean.length === 12) {
    return `${clean.slice(0, 4)}-${clean.slice(4, 8)}-${clean.slice(8, 12)}`;
  }
  return code;
}

/** Parse user input -- accepts both 7-char and 12-char codes. Returns raw code or null. */
export function parseSpaceCode(input: string): string | null {
  const clean = input.replace(/[-\s]/g, "").toUpperCase();
  if (clean.length === 7 || clean.length === 12) {
    if (![...clean].every((c) => CROCKFORD.includes(c))) return null;
    return clean;
  }
  return null;
}

// ---------------------------------------------------------------------------
// QR payload generation and parsing
// ---------------------------------------------------------------------------

/**
 * Generate a QR payload string for sharing a space.
 * Format: ark://join?code=XXXX-XXXX-XXXX&addrs=addr1,addr2
 */
export function generateQrPayload(code: string, addresses: string[]): string {
  const formatted = formatSpaceCode(code);
  const addrs = addresses.join(",");
  return `ark://join?code=${formatted}&addrs=${addrs}`;
}

/**
 * Parse a QR payload string back to code + addresses.
 * Returns null if the payload is invalid.
 */
export function parseQrPayload(
  payload: string,
): { code: string; addresses: string[] } | null {
  // Accept both ark://join?... and raw codes
  if (payload.startsWith("ark://join?")) {
    try {
      // Parse as URL params after ark://join?
      const queryString = payload.slice("ark://join?".length);
      const params = new URLSearchParams(queryString);
      const rawCode = params.get("code");
      const rawAddrs = params.get("addrs");

      if (!rawCode) return null;
      const code = parseSpaceCode(rawCode);
      if (!code) return null;

      const addresses = rawAddrs
        ? rawAddrs.split(",").filter((a) => a.length > 0)
        : [];

      return { code, addresses };
    } catch {
      return null;
    }
  }

  // Fallback: try parsing as a raw space code
  const code = parseSpaceCode(payload);
  if (code) return { code, addresses: [] };

  return null;
}

// ---------------------------------------------------------------------------
// Own addresses (called via IPC from main process)
// ---------------------------------------------------------------------------

/**
 * Collect all non-loopback IP addresses from network interfaces.
 * This function uses Node.js `os` module and must run in the Electron main process.
 * In the renderer, use IPC to call sync:getOwnAddresses.
 */
export function getOwnAddresses(port: number): string[] {
  // eslint-disable-next-line @typescript-eslint/no-require-imports
  const os = require("node:os");
  const addresses: string[] = [];
  const interfaces = os.networkInterfaces();
  for (const [name, nets] of Object.entries(interfaces)) {
    for (const net of (nets as Array<{
      internal: boolean;
      family: string;
      address: string;
    }>) ?? []) {
      if (net.internal) continue;
      if (net.family === "IPv4") {
        addresses.push(`${net.address}:${port}`);
      } else if (net.family === "IPv6") {
        addresses.push(`[${net.address}%${name}]:${port}`);
      }
    }
  }
  return addresses;
}

// ---------------------------------------------------------------------------
// Space persistence (localStorage)
// ---------------------------------------------------------------------------

/**
 * Derive a stable space ID from a code.
 * spaceId = SHA-256(uppercase raw code) -> first 16 hex characters.
 */
export async function deriveSpaceId(code: string): Promise<string> {
  const raw = code.replace(/[-\s]/g, "").toUpperCase();
  const data = new TextEncoder().encode(raw);
  const hashBuffer = await crypto.subtle.digest("SHA-256", data);
  const hashArray = Array.from(new Uint8Array(hashBuffer));
  const hex = hashArray.map((b) => b.toString(16).padStart(2, "0")).join("");
  return hex.slice(0, 16);
}

/** Get all saved spaces. */
export function getSpaces(): Space[] {
  try {
    return JSON.parse(localStorage.getItem(SPACES_KEY) || "[]");
  } catch {
    return [];
  }
}

/** Save a space to the list. */
export function saveSpace(space: Space): void {
  const spaces = getSpaces().filter((s) => s.code !== space.code);
  spaces.unshift(space);
  localStorage.setItem(SPACES_KEY, JSON.stringify(spaces));
}

/** Remove a space from the list. */
export function removeSpace(code: string): void {
  const spaces = getSpaces().filter((s) => s.code !== code);
  localStorage.setItem(SPACES_KEY, JSON.stringify(spaces));
}

/** Get the currently active space code (raw, no dashes). */
export function getActiveSpace(): string | null {
  return localStorage.getItem(ACTIVE_SPACE_KEY) || null;
}

/** Set the active space. */
export function setActiveSpace(code: string | null): void {
  if (code) {
    localStorage.setItem(ACTIVE_SPACE_KEY, code);
  } else {
    localStorage.removeItem(ACTIVE_SPACE_KEY);
  }
}
