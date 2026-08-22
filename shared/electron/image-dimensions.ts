export type ImageDimensions = { width: number; height: number };

const isJpegStartOfFrame = (marker: number) =>
  marker >= 0xc0 && marker <= 0xcf && marker !== 0xc4 && marker !== 0xc8 && marker !== 0xcc;

const uint16 = (bytes: Uint8Array, offset: number) => bytes[offset]! * 256 + bytes[offset + 1]!;

const uint32 = (bytes: Uint8Array, offset: number) =>
  bytes[offset]! * 0x1000000 +
  bytes[offset + 1]! * 0x10000 +
  bytes[offset + 2]! * 0x100 +
  bytes[offset + 3]!;

export function imageDimensions(bytes: Uint8Array): ImageDimensions | null {
  if (
    bytes.length >= 24 &&
    bytes[0] === 0x89 &&
    bytes[1] === 0x50 &&
    bytes[2] === 0x4e &&
    bytes[3] === 0x47 &&
    bytes[4] === 0x0d &&
    bytes[5] === 0x0a &&
    bytes[6] === 0x1a &&
    bytes[7] === 0x0a &&
    uint32(bytes, 8) === 13 &&
    bytes[12] === 0x49 &&
    bytes[13] === 0x48 &&
    bytes[14] === 0x44 &&
    bytes[15] === 0x52
  ) {
    const width = uint32(bytes, 16);
    const height = uint32(bytes, 20);
    return width && height ? { width, height } : null;
  }

  if (bytes.length < 4 || bytes[0] !== 0xff || bytes[1] !== 0xd8) return null;

  let offset = 2;
  while (offset < bytes.length) {
    if (bytes[offset++] !== 0xff) return null;
    while (bytes[offset] === 0xff) offset++;
    if (offset >= bytes.length) return null;

    const marker = bytes[offset++]!;
    if (marker === 0xd9 || marker === 0xda || marker === 0x00) return null;
    if (marker === 0x01 || marker === 0xd8 || (marker >= 0xd0 && marker <= 0xd7)) continue;
    if (offset + 2 > bytes.length) return null;

    const length = uint16(bytes, offset);
    const end = offset + length;
    if (length < 2 || end > bytes.length) return null;
    if (isJpegStartOfFrame(marker)) {
      if (length < 7) return null;
      const height = uint16(bytes, offset + 3);
      const width = uint16(bytes, offset + 5);
      return width && height ? { width, height } : null;
    }
    offset = end;
  }

  return null;
}
