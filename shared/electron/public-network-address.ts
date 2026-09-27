import { isIP } from "node:net";

// IPv6 grammar allows many spellings of the same address ("::ffff:8.8.8.8",
// "0:0:0:0:0:ffff:808:808", "0::ffff:808:808", ...). String-prefix checks
// cannot recognize them all, so parse into hextets like the Rust port does.
function ipv6Segments(address: string): number[] | null {
  let head = address;
  const tailSegments: number[] = [];
  if (head.includes(".")) {
    const lastColon = head.lastIndexOf(":");
    if (lastColon === -1) return null;
    const quad = head
      .slice(lastColon + 1)
      .split(".")
      .map(Number);
    if (
      quad.length !== 4 ||
      quad.some((part) => !Number.isInteger(part) || part < 0 || part > 255)
    ) {
      return null;
    }
    head = head.slice(0, lastColon);
    tailSegments.push(((quad[0]! << 8) | quad[1]!) >>> 0, ((quad[2]! << 8) | quad[3]!) >>> 0);
  }
  const halves = head.split("::");
  if (halves.length > 2) return null;
  const toHextets = (half: string): number[] | null => {
    if (!half) return [];
    const parts = half.split(":").map((part) => Number.parseInt(part, 16));
    return parts.some((part) => !Number.isInteger(part) || part < 0 || part > 0xffff)
      ? null
      : parts;
  };
  const left = toHextets(halves[0]!);
  const right = toHextets(halves.length === 2 ? halves[1]! : "");
  if (!left || !right) return null;
  const explicit = left.length + right.length + tailSegments.length;
  if (halves.length === 2 ? explicit > 7 : explicit !== 8) return null;
  return [...left, ...new Array<number>(8 - explicit).fill(0), ...right, ...tailSegments];
}

export function isPublicNetworkAddress(input: string): boolean {
  const address = input
    .trim()
    .replace(/^\[|\]$/g, "")
    .toLowerCase();
  const version = isIP(address);
  if (version === 4) {
    const [a, b, c] = address.split(".").map(Number);
    return !(
      a === 0 ||
      a === 10 ||
      a === 127 ||
      (a === 100 && b! >= 64 && b! <= 127) ||
      (a === 169 && b === 254) ||
      (a === 172 && b! >= 16 && b! <= 31) ||
      (a === 192 && b === 0 && c === 0) ||
      (a === 192 && b === 0 && c === 2) ||
      (a === 192 && b === 168) ||
      (a === 198 && (b === 18 || b === 19)) ||
      (a === 198 && b === 51 && c === 100) ||
      (a === 203 && b === 0 && c === 113) ||
      a! >= 224
    );
  }
  if (version !== 6) return false;
  const segments = ipv6Segments(address);
  if (!segments) return false;
  // IPv4-mapped IPv6 (::ffff:a.b.c.d) in every spelling.
  if (segments.slice(0, 5).every((segment) => segment === 0) && segments[5] === 0xffff) {
    return isPublicNetworkAddress(
      `${segments[6]! >> 8}.${segments[6]! & 0xff}.${segments[7]! >> 8}.${segments[7]! & 0xff}`,
    );
  }
  const [first = 0, second = 0] = segments;
  return !(
    first === 0 ||
    (first & 0xfe00) === 0xfc00 ||
    (first & 0xffc0) === 0xfe80 ||
    (first & 0xff00) === 0xff00 ||
    (first === 0x2001 && second === 0x0db8)
  );
}
