/**
 * Delphi Space Manager — space code generation, encoding, QR payloads,
 * and Delphi-specific persistence.
 *
 * On Electron: persists via IPC to main process (JSON file in appData — survives localStorage wipe).
 * On Web: falls back to localStorage.
 *
 * Space functions are now inlined (arksync package removed).
 */

// ---------------------------------------------------------------------------
// Space code utilities (inlined from arksync/src/space.ts)
// ---------------------------------------------------------------------------

const CROCKFORD = "0123456789ABCDEFGHJKMNPQRSTVWXYZ"; // no I, L, O, U
const LAN_PORT = 21531;

export function generateSpaceCode(): string {
  const arr = new Uint8Array(12);
  crypto.getRandomValues(arr);
  return Array.from(arr, (b) => CROCKFORD[b % 32]).join("");
}

export function encodeIpv4(ipv4: string): string | null {
  const m = ipv4.match(/^(\d{1,3})\.(\d{1,3})\.(\d{1,3})\.(\d{1,3})$/);
  if (!m) return null;
  const parts = [+m[1], +m[2], +m[3], +m[4]];
  if (parts.some((n) => n < 0 || n > 255)) return null;
  const n = BigInt(
    ((parts[0] << 24) | (parts[1] << 16) | (parts[2] << 8) | parts[3]) >>> 0,
  );
  const shifted = n << 3n;
  let result = "";
  for (let i = 6; i >= 0; i--) {
    result = CROCKFORD[Number((shifted >> BigInt(i * 5)) & 0x1fn)] + result;
  }
  return result;
}

export function decodeIpv4(encoded: string): string | null {
  const clean = encoded.toUpperCase();
  if (clean.length !== 7) return null;
  if (![...clean].every((c) => CROCKFORD.includes(c))) return null;
  let value = 0n;
  for (const c of clean) {
    value = (value << 5n) | BigInt(CROCKFORD.indexOf(c));
  }
  value = value >> 3n;
  const parts = [
    Number((value >> 24n) & 0xffn),
    Number((value >> 16n) & 0xffn),
    Number((value >> 8n) & 0xffn),
    Number(value & 0xffn),
  ];
  return `${parts[0]}.${parts[1]}.${parts[2]}.${parts[3]}`;
}

export function generateExtendedCode(
  code: string,
  primaryIpv4: string,
): string | null {
  const encoded = encodeIpv4(primaryIpv4);
  if (!encoded) return null;
  return code.replace(/[-\s]/g, "").toUpperCase().slice(0, 12) + encoded;
}

export function formatSpaceCode(code: string): string {
  const clean = code.replace(/[-\s]/g, "").toUpperCase();
  if (clean.length === 7) {
    return `${clean.slice(0, 4)}-${clean.slice(4, 7)}`;
  }
  if (clean.length === 12) {
    return `${clean.slice(0, 4)}-${clean.slice(4, 8)}-${clean.slice(8, 12)}`;
  }
  if (clean.length === 19) {
    return `${clean.slice(0, 4)}-${clean.slice(4, 8)}-${clean.slice(8, 12)}-${clean.slice(12, 16)}-${clean.slice(16, 19)}`;
  }
  return code;
}

export function parseSpaceCode(input: string): string | null {
  const clean = input.replace(/[-\s]/g, "").toUpperCase();
  if (clean.length === 7 || clean.length === 12) {
    if (![...clean].every((c) => CROCKFORD.includes(c))) return null;
    return clean;
  }
  return null;
}

export function generateQrPayload(code: string, addresses: string[]): string {
  const formatted = formatSpaceCode(code);
  const addrs = addresses.join(",");
  return `ark://join?code=${formatted}&addrs=${addrs}`;
}

export function parseQrPayload(
  payload: string,
): { code: string; addresses: string[] } | null {
  if (payload.startsWith("ark://join?")) {
    try {
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

  // Extended 19-char code
  const clean = payload.replace(/[-\s]/g, "").toUpperCase();
  if (clean.length === 19 && [...clean].every((c) => CROCKFORD.includes(c))) {
    const code = clean.slice(0, 12);
    const ipv4 = decodeIpv4(clean.slice(12));
    const addresses = ipv4 ? [`${ipv4}:${LAN_PORT}`] : [];
    return { code, addresses };
  }

  const code = parseSpaceCode(payload);
  if (code) return { code, addresses: [] };

  return null;
}

export async function deriveSpaceId(code: string): Promise<string> {
  const raw = code.replace(/[-\s]/g, "").toUpperCase();
  const data = new TextEncoder().encode(raw);
  const hashBuffer = await crypto.subtle.digest("SHA-256", data);
  const hashArray = Array.from(new Uint8Array(hashBuffer));
  const hex = hashArray.map((b) => b.toString(16).padStart(2, "0")).join("");
  return hex.slice(0, 16);
}

/** Internal alias used by renameSpace below */
const _formatSpaceCode = formatSpaceCode;

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

const isElectron =
  typeof window !== "undefined" &&
  !!(window as unknown as Record<string, unknown>).electronAPI;

function ipc(): {
  invoke: (channel: string, ...args: unknown[]) => Promise<unknown>;
} | null {
  if (typeof window === "undefined") return null;
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
