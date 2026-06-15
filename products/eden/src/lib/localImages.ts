const LOCAL_IMAGE_PROTOCOL = "kosmos-local-image";

export function localImageUrl(filePath: string): string {
  return `${LOCAL_IMAGE_PROTOCOL}://file/${encodeURIComponent(filePath)}`;
}

export function toDisplayImageSrc(src: string): string {
  const trimmed = src.trim();
  if (/^[A-Za-z]:[\\/]/.test(trimmed) || trimmed.startsWith("\\\\")) {
    return localImageUrl(trimmed);
  }

  if (!trimmed.startsWith("file:")) {
    return trimmed;
  }

  try {
    const url = new URL(trimmed);
    const decodedPath = decodeURIComponent(url.pathname);
    const windowsPath = decodedPath.replace(/^\/([A-Za-z]:)/, "$1");
    return localImageUrl(windowsPath);
  } catch {
    return trimmed;
  }
}
