import path from "node:path";
import { existsSync, readdirSync } from "node:fs";

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

function userHomeDir(): string | null {
  const home = process.env.USERPROFILE || process.env.HOME;
  return home && home.trim().length > 0 ? home : null;
}

function markdownBrainSuffix(filePath: string): string | null {
  const normalized = path.normalize(filePath);
  const parts = normalized.split(/[\\/]+/);
  const index = parts.findIndex((part) => part.toLocaleLowerCase("ru") === "markdown-brain");
  if (index < 0 || index >= parts.length - 1) return null;
  return path.join(...parts.slice(index + 1));
}

function candidateExistingPath(filePath: string): string | null {
  if (existsSync(filePath)) return filePath;

  const suffix = markdownBrainSuffix(filePath);
  const home = userHomeDir();
  if (!suffix || !home) return null;

  const candidates: string[] = [];
  const desktop = path.join(home, "Desktop");
  candidates.push(path.join(desktop, "markdown-brain", suffix));
  candidates.push(path.join(home, "Yandex.Disk", "markdown-brain", suffix));

  try {
    for (const entry of readdirSync(desktop, { withFileTypes: true })) {
      if (!entry.isDirectory()) continue;
      candidates.push(path.join(desktop, entry.name, "markdown-brain", suffix));
    }
  } catch {
    // Desktop may be unavailable in test/service contexts.
  }

  return candidates.find((candidate) => existsSync(candidate)) ?? null;
}

export function resolveLocalImagePath(filePath: string): string | null {
  if (!isSupportedLocalImagePath(filePath)) return null;
  return candidateExistingPath(filePath);
}

function isSupportedLocalImagePath(filePath: string): boolean {
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
