import type { ObsidianVaultImageFile } from "./obsidianVault";

const IMAGE_MARKDOWN_RE = /!\[([^\]]*)\]\(([^)\n]+)\)|!\[\[([^|\]]+)(?:\|([^\]]+))?\]\]/g;

export function buildImageAssetMap(
  images: readonly ObsidianVaultImageFile[],
): Map<string, ObsidianVaultImageFile> {
  const map = new Map<string, ObsidianVaultImageFile>();
  for (const image of images) {
    const relative = normalizePath(image.relativePath);
    map.set(relative.toLocaleLowerCase("ru"), image);
    map.set(image.name.toLocaleLowerCase("ru"), image);
    map.set(encodeURI(relative).toLocaleLowerCase("ru"), image);
  }
  return map;
}

export function extractMarkdownImageRefs(markdown: string): string[] {
  return [...markdown.matchAll(IMAGE_MARKDOWN_RE)]
    .map((match) => parseMarkdownImageTarget(match[2] ?? match[3] ?? ""))
    .filter(Boolean);
}

export function rewriteImageReferences(
  markdown: string,
  sourceRelativePath: string,
  imageAssets: Map<string, ObsidianVaultImageFile>,
): string {
  const sourceDir = sourceRelativePath.includes("/")
    ? sourceRelativePath.slice(0, sourceRelativePath.lastIndexOf("/"))
    : "";

  return markdown.replace(
    IMAGE_MARKDOWN_RE,
    (raw, alt: string, inlineSrc: string, wikiSrc: string) => {
      const src = parseMarkdownImageTarget(inlineSrc ?? wikiSrc ?? "");
      const asset = resolveImageAsset(src, sourceDir, imageAssets);
      if (!asset) return raw;
      const label = (alt || titleFromPath(asset.relativePath)).trim();
      return `![${label}](${asset.fileUrl})`;
    },
  );
}

export function titleFromPath(relativePath: string): string {
  const fileName = normalizePath(relativePath).split("/").pop() ?? relativePath;
  return fileName.replace(/\.[^.]+$/, "").trim() || "Без названия";
}

export function stableIdFromPath(value: string): string {
  let hash = 0x811c9dc5;
  for (const char of normalizePath(value)) {
    hash ^= char.charCodeAt(0);
    hash = Math.imul(hash, 0x01000193);
  }
  return hash.toString(16).padStart(8, "0");
}

function resolveImageAsset(
  src: string,
  sourceDir: string,
  imageAssets: Map<string, ObsidianVaultImageFile>,
): ObsidianVaultImageFile | null {
  if (/^[a-z]+:/i.test(src)) return null;
  const normalized = normalizePath(decodeURIComponent(src));
  const candidates = [
    normalized,
    normalizePath(`${sourceDir}/${normalized}`),
    normalized.split("/").pop() ?? normalized,
  ];

  for (const candidate of candidates) {
    const asset = imageAssets.get(candidate.toLocaleLowerCase("ru"));
    if (asset) return asset;
  }
  return null;
}

function parseMarkdownImageTarget(value: string): string {
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
