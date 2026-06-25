export interface ObsidianExportAssetPlan {
  targetRelativePath: string;
  sourcePath: string | null;
  assetKey: string | null;
}

export function safeVaultPathSegment(value: string): string {
  const safe = value
    .trim()
    .replace(/[<>:"/\\|?*]/g, "-")
    .split("")
    .map((char) => (char.charCodeAt(0) < 32 ? "-" : char))
    .join("")
    .replace(/\.+$/g, "")
    .replace(/^\.+/g, "")
    .replace(/\s+/g, " ")
    .trim();
  return safe || "folder";
}

export function deriveObsidianExportAssetPlan(
  source: string,
  options: { preferredFileName?: string | null; mimeType?: string | null } = {},
): ObsidianExportAssetPlan | null {
  const trimmed = source.trim();
  if (!trimmed) return null;
  if (/^(https?|mailto|data):/i.test(trimmed)) return null;

  const rawPath = extractLocalAssetPath(trimmed);
  if (!rawPath) return null;

  const normalized = normalizePath(rawPath);
  if (!normalized) return null;

  const pathParts = normalized.split("/").filter(Boolean);
  if (pathParts.length === 0) return null;

  const sourcePath = resolveCopyableLocalAssetSourcePath(trimmed);
  return {
    targetRelativePath: buildObsidianExportAssetTargetRelativePath(trimmed, pathParts, options),
    sourcePath,
    assetKey: sourcePath ? canonicalLocalAssetKey(sourcePath) : null,
  };
}

export function allocateUniqueExportPath(relativePath: string, usedPaths: Set<string>): string {
  const key = relativePath.toLocaleLowerCase("ru");
  if (!usedPaths.has(key)) {
    usedPaths.add(key);
    return relativePath;
  }

  const lastSlash = relativePath.lastIndexOf("/");
  const folder = lastSlash >= 0 ? relativePath.slice(0, lastSlash + 1) : "";
  const fileName = lastSlash >= 0 ? relativePath.slice(lastSlash + 1) : relativePath;
  const dotIndex = fileName.lastIndexOf(".");
  const stem = dotIndex > 0 ? fileName.slice(0, dotIndex) : fileName;
  const ext = dotIndex > 0 ? fileName.slice(dotIndex) : "";

  for (let suffix = 2; suffix < 1000; suffix += 1) {
    const candidate = `${folder}${stem}-${suffix}${ext}`;
    const candidateKey = candidate.toLocaleLowerCase("ru");
    if (!usedPaths.has(candidateKey)) {
      usedPaths.add(candidateKey);
      return candidate;
    }
  }

  throw new Error(
    "[kepler-shell] Markdown vault export asset path collision could not be resolved",
  );
}

export function relativePathBetween(fromDir: string, toPath: string): string {
  const fromParts = fromDir ? fromDir.split("/").filter(Boolean) : [];
  const toParts = toPath.split("/").filter(Boolean);
  let shared = 0;
  while (
    shared < fromParts.length &&
    shared < toParts.length &&
    fromParts[shared] === toParts[shared]
  ) {
    shared += 1;
  }

  const up = fromParts.length - shared;
  const down = toParts.slice(shared);
  return [...Array(up).fill(".."), ...down].join("/") || ".";
}

export function parseMarkdownImageTarget(value: string): string {
  const trimmed = value.trim();
  if (!trimmed) return "";
  if (trimmed.startsWith("<")) {
    const closeIndex = trimmed.indexOf(">");
    if (closeIndex > 1) return trimmed.slice(1, closeIndex).trim();
  }

  const titleMatch = trimmed.match(/\s+(?:"[^"]*"|'[^']*')\s*$/);
  if (titleMatch?.index && titleMatch.index > 0) {
    return trimmed.slice(0, titleMatch.index).trim();
  }

  return trimmed;
}

export function titleFromPath(relativePath: string): string {
  const fileName = normalizePath(relativePath).split("/").pop() ?? relativePath;
  return fileName.replace(/\.[^.]+$/, "").trim() || "Без названия";
}

function buildObsidianExportAssetTargetRelativePath(
  source: string,
  pathParts: string[],
  options: { preferredFileName?: string | null; mimeType?: string | null },
): string {
  const isAbsolutePath =
    /^file:/i.test(source) ||
    /^kosmos-local-image:/i.test(source) ||
    /^[A-Za-z]:[\\/]/.test(source) ||
    source.startsWith("/") ||
    source.startsWith("\\\\");
  const sourceLeaf = pathParts[pathParts.length - 1] ?? "asset";
  const preferredLeaf = fileNameFromPathLike(options.preferredFileName ?? "");

  if (preferredLeaf) {
    return `assets/${finalizeAssetFileName(preferredLeaf, sourceLeaf, options.mimeType)}`;
  }

  if (isAbsolutePath) {
    return `assets/${finalizeAssetFileName(sourceLeaf, null, options.mimeType)}`;
  }

  return `assets/${pathParts
    .map((part, index) =>
      index === pathParts.length - 1
        ? finalizeAssetFileName(part, null, options.mimeType)
        : safeVaultPathSegment(part),
    )
    .join("/")}`;
}

function finalizeAssetFileName(
  candidate: string,
  fallbackExtensionSource: string | null,
  mimeType?: string | null,
): string {
  const safeCandidate = safeVaultPathSegment(fileNameFromPathLike(candidate) || "asset");
  if (fileExtension(safeCandidate)) return safeCandidate;

  const fallbackExtension =
    (fallbackExtensionSource ? fileExtension(fallbackExtensionSource) : "") ||
    imageExtensionFromMime(mimeType);
  return fallbackExtension ? `${safeCandidate}${fallbackExtension}` : safeCandidate;
}

function fileNameFromPathLike(value: string): string {
  const trimmed = value.trim();
  if (!trimmed) return "";
  return trimmed.replace(/\\/g, "/").split("/").filter(Boolean).pop() ?? "";
}

function fileExtension(fileName: string): string {
  const dotIndex = fileName.lastIndexOf(".");
  if (dotIndex <= 0 || dotIndex === fileName.length - 1) return "";
  return fileName.slice(dotIndex);
}

function imageExtensionFromMime(mimeType: string | null | undefined): string {
  const normalized = mimeType?.trim().toLocaleLowerCase("ru") ?? "";
  if (normalized === "image/jpeg" || normalized === "image/jpg") return ".jpg";
  if (normalized === "image/png") return ".png";
  if (normalized === "image/webp") return ".webp";
  if (normalized === "image/gif") return ".gif";
  if (normalized === "image/avif") return ".avif";
  return "";
}

function canonicalLocalAssetKey(sourcePath: string): string {
  const resolvedPath = extractCopyableLocalAssetFilesystemPath(sourcePath) ?? sourcePath.trim();
  return normalizePath(resolvedPath).toLocaleLowerCase("ru");
}

function extractCopyableLocalAssetFilesystemPath(sourcePath: string): string | null {
  const trimmed = sourcePath.trim();
  if (!trimmed) return null;

  if (/^file:/i.test(trimmed)) {
    try {
      const url = new URL(trimmed);
      return decodeLocalAssetUrlPath(url.pathname);
    } catch {
      return null;
    }
  }

  if (/^kosmos-local-image:/i.test(trimmed)) {
    try {
      const url = new URL(trimmed);
      return decodeLocalAssetUrlPath(url.pathname);
    } catch {
      return null;
    }
  }

  return trimmed;
}

function decodeLocalAssetUrlPath(pathname: string): string {
  const decoded = decodeURIComponent(pathname || "");
  return decoded.replace(/^\/([A-Za-z]:[\\/])/, "$1").replace(/^\/+/, "/");
}

function resolveCopyableLocalAssetSourcePath(source: string): string | null {
  const trimmed = source.trim();
  if (!trimmed) return null;

  if (/^file:/i.test(trimmed) || /^kosmos-local-image:/i.test(trimmed)) {
    try {
      new URL(trimmed);
      return trimmed;
    } catch {
      return null;
    }
  }

  if (/^[A-Za-z]:[\\/]/.test(trimmed) || trimmed.startsWith("\\\\") || trimmed.startsWith("/")) {
    return trimmed;
  }

  return null;
}

function extractLocalAssetPath(source: string): string | null {
  const trimmed = source.trim();
  if (!trimmed) return null;

  if (/^file:/i.test(trimmed) || /^kosmos-local-image:/i.test(trimmed)) {
    try {
      const url = new URL(trimmed);
      const decoded = decodeURIComponent(url.pathname || "").replace(/^\/+/, "");
      return decoded || null;
    } catch {
      return null;
    }
  }

  if (
    /^[A-Za-z]:[\\/]/.test(trimmed) ||
    trimmed.startsWith("\\\\") ||
    trimmed.startsWith("/") ||
    trimmed.startsWith("//")
  ) {
    return trimmed;
  }

  if (/^[a-z]+:/i.test(trimmed)) {
    return null;
  }

  return trimmed;
}

function normalizePath(value: string): string {
  const parts: string[] = [];
  for (const part of value.replace(/\\/g, "/").replace(/^\/+/, "").split("/")) {
    if (!part || part === ".") continue;
    if (part === "..") {
      parts.pop();
      continue;
    }
    parts.push(part);
  }
  return parts.join("/");
}
