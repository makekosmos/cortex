export const APP_ICON_PROTOCOL = "kosmos-icon";

export function parseAppIconRequestUrl(rawUrl: string): string | null {
  let url: URL;
  try {
    url = new URL(rawUrl);
  } catch {
    return null;
  }
  if (url.protocol !== `${APP_ICON_PROTOCOL}:` || url.hostname !== "app") {
    return null;
  }
  const encodedId = url.pathname.replace(/^\/+/, "");
  if (!encodedId) return null;
  try {
    return decodeURIComponent(encodedId);
  } catch {
    return null;
  }
}

export function bufferToArrayBuffer(buffer: Buffer): ArrayBuffer {
  const out = new ArrayBuffer(buffer.byteLength);
  new Uint8Array(out).set(buffer);
  return out;
}
