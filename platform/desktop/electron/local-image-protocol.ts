import path from "node:path";

export const LOCAL_IMAGE_PROTOCOL = "kosmos-local-image";

const LOCAL_IMAGE_EXTENSIONS = new Set([".png", ".jpg", ".jpeg", ".gif", ".webp", ".avif"]);

export function localImageUrl(filePath: string): string {
  return `${LOCAL_IMAGE_PROTOCOL}://file/${encodeURIComponent(filePath)}`;
}

export function parseLocalImageRequestUrl(rawUrl: string): string | null {
  let url: URL;
  try {
    url = new URL(rawUrl);
  } catch {
    return null;
  }

  if (url.protocol !== `${LOCAL_IMAGE_PROTOCOL}:` || url.hostname !== "file") {
    return null;
  }

  const encodedPath = url.pathname.replace(/^\/+/, "");
  if (!encodedPath) return null;

  let filePath: string;
  try {
    filePath = decodeURIComponent(encodedPath);
  } catch {
    return null;
  }

  return isSupportedLocalImagePath(filePath) ? filePath : null;
}

export function isSupportedLocalImagePath(filePath: string): boolean {
  return LOCAL_IMAGE_EXTENSIONS.has(path.extname(filePath).toLowerCase());
}

export function localImageMimeType(filePath: string): string {
  switch (path.extname(filePath).toLowerCase()) {
    case ".png":
      return "image/png";
    case ".jpg":
    case ".jpeg":
      return "image/jpeg";
    case ".gif":
      return "image/gif";
    case ".webp":
      return "image/webp";
    case ".avif":
      return "image/avif";
    default:
      return "application/octet-stream";
  }
}
