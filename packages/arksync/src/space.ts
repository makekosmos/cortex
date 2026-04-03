/**
 * ArkSync Space Manager — space code generation, encoding, QR payloads.
 *
 * v3: 12-character random Base32-Crockford code, format XXXX-XXXX-XXXX.
 * v4: 19-character extended code = 12-char secret + 7-char encoded IPv4,
 *     format XXXX-XXXX-XXXX-XXXX-XXX. Encodes primary LAN IP for codeless join.
 * All peers are equal — the code is used as HMAC secret for auth.
 *
 * QR payload: ark://join?code=XXXX-XXXX-XXXX&addrs=addr1,addr2
 */

const CROCKFORD = "0123456789ABCDEFGHJKMNPQRSTVWXYZ"; // no I, L, O, U
const LAN_PORT = 21531;

/** Generate a random 12-character Base32-Crockford code. */
export function generateSpaceCode(): string {
  const arr = new Uint8Array(12);
  crypto.getRandomValues(arr);
  return Array.from(arr, (b) => CROCKFORD[b % 32]).join("");
}

/**
 * Encode an IPv4 address string ("192.168.1.70") into 7 Base32-Crockford chars.
 */
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

/**
 * Decode 7 Base32-Crockford chars back to an IPv4 string ("192.168.1.70").
 */
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

/**
 * Build an extended 19-char code = 12-char secret + 7-char encoded IPv4.
 */
export function generateExtendedCode(
  code: string,
  primaryIpv4: string,
): string | null {
  const encoded = encodeIpv4(primaryIpv4);
  if (!encoded) return null;
  return code.replace(/[-\s]/g, "").toUpperCase().slice(0, 12) + encoded;
}

/**
 * Format code with dashes — handles legacy 7-char, standard 12-char,
 * and extended 19-char (XXXX-XXXX-XXXX-XXXX-XXX) codes.
 */
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

/** Parse user input — accepts 7-char and 12-char codes. Returns raw 12-char code or null. */
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

// ---------------------------------------------------------------------------
// Space ID derivation
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

// getOwnAddresses moved to node.ts — requires Node.js `os` module, not browser-safe
