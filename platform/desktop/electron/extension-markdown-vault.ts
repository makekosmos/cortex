import { readFileSync, readdirSync, statSync } from "node:fs";
import path from "node:path";
import { fileURLToPath } from "node:url";
import {
  localImageMimeType,
  localImageUrl,
  parseLocalImageRequestUrl,
} from "./local-image-protocol";

export const MARKDOWN_FILE_MAX_BYTES = 5 * 1024 * 1024;

const MARKDOWN_VAULT_MAX_FILES = 5_000;
const MARKDOWN_VAULT_MAX_IMAGES = 5_000;
const MARKDOWN_IMAGE_EXTENSIONS = new Set([".png", ".jpg", ".jpeg", ".gif", ".webp", ".avif"]);

type MarkdownVaultTextFile = {
  path: string;
  relativePath: string;
  name: string;
  content: string;
};

type MarkdownVaultImageFile = {
  path: string;
  relativePath: string;
  name: string;
  fileUrl: string;
  mimeType: string;
  sizeBytes: number;
  width: number | null;
  height: number | null;
};

export type MarkdownVaultOpenResult = {
  rootPath: string;
  files: MarkdownVaultTextFile[];
  images: MarkdownVaultImageFile[];
};

export type MarkdownVaultExportFile =
  | {
      relativePath: string;
      content: string;
      sourcePath?: never;
    }
  | {
      relativePath: string;
      sourcePath: string;
      content?: never;
    };

export function safeMarkdownDefaultName(name: unknown): string {
  const fallback = "eden-object.md";
  if (typeof name !== "string") return fallback;
  const base = path
    .basename(name)
    .replace(/[<>:"/\\|?*]/g, "-")
    .replace(/./g, (char) => (char.charCodeAt(0) < 32 ? "-" : char))
    .trim();
  if (!base) return fallback;
  return base.toLowerCase().endsWith(".md") ? base : `${base}.md`;
}

function normalizeVaultRelativePath(root: string, filePath: string): string {
  return path.relative(root, filePath).split(path.sep).join("/");
}

function isIgnoredVaultDir(name: string): boolean {
  return name.startsWith(".") || name === "node_modules";
}

function readUInt24LE(buffer: Buffer, offset: number): number {
  return buffer[offset] + (buffer[offset + 1] << 8) + (buffer[offset + 2] << 16);
}

function readImageDimensions(filePath: string): { width: number; height: number } | null {
  try {
    const buffer = readFileSync(filePath);
    if (
      buffer.length >= 24 &&
      buffer[0] === 0x89 &&
      buffer[1] === 0x50 &&
      buffer[2] === 0x4e &&
      buffer[3] === 0x47
    ) {
      return { width: buffer.readUInt32BE(16), height: buffer.readUInt32BE(20) };
    }

    if (buffer.length >= 10 && buffer.toString("ascii", 0, 6) === "GIF87a") {
      return { width: buffer.readUInt16LE(6), height: buffer.readUInt16LE(8) };
    }
    if (buffer.length >= 10 && buffer.toString("ascii", 0, 6) === "GIF89a") {
      return { width: buffer.readUInt16LE(6), height: buffer.readUInt16LE(8) };
    }

    if (buffer.length >= 12 && buffer[0] === 0xff && buffer[1] === 0xd8) {
      let offset = 2;
      while (offset + 9 < buffer.length) {
        if (buffer[offset] !== 0xff) {
          offset += 1;
          continue;
        }
        const marker = buffer[offset + 1];
        const size = buffer.readUInt16BE(offset + 2);
        if (size < 2) return null;
        if (
          (marker >= 0xc0 && marker <= 0xc3) ||
          (marker >= 0xc5 && marker <= 0xc7) ||
          (marker >= 0xc9 && marker <= 0xcb) ||
          (marker >= 0xcd && marker <= 0xcf)
        ) {
          return {
            height: buffer.readUInt16BE(offset + 5),
            width: buffer.readUInt16BE(offset + 7),
          };
        }
        offset += 2 + size;
      }
    }

    if (
      buffer.length >= 30 &&
      buffer.toString("ascii", 0, 4) === "RIFF" &&
      buffer.toString("ascii", 8, 12) === "WEBP"
    ) {
      const chunk = buffer.toString("ascii", 12, 16);
      if (chunk === "VP8X" && buffer.length >= 30) {
        return {
          width: readUInt24LE(buffer, 24) + 1,
          height: readUInt24LE(buffer, 27) + 1,
        };
      }
      if (chunk === "VP8 " && buffer.length >= 30) {
        return {
          width: buffer.readUInt16LE(26) & 0x3fff,
          height: buffer.readUInt16LE(28) & 0x3fff,
        };
      }
      if (chunk === "VP8L" && buffer.length >= 25) {
        const bits = buffer.readUInt32LE(21);
        return {
          width: (bits & 0x3fff) + 1,
          height: ((bits >> 14) & 0x3fff) + 1,
        };
      }
    }
  } catch {
    return null;
  }

  return null;
}

export function scanMarkdownVault(rootPath: string): MarkdownVaultOpenResult {
  const files: MarkdownVaultTextFile[] = [];
  const images: MarkdownVaultImageFile[] = [];

  const visit = (dir: string) => {
    const entries = readdirSync(dir, { withFileTypes: true });
    for (const entry of entries) {
      const fullPath = path.join(dir, entry.name);
      if (entry.isDirectory()) {
        if (!isIgnoredVaultDir(entry.name)) visit(fullPath);
        continue;
      }
      if (!entry.isFile()) continue;

      const ext = path.extname(entry.name).toLowerCase();
      if (ext === ".md" || ext === ".markdown") {
        if (files.length >= MARKDOWN_VAULT_MAX_FILES) continue;
        const stat = statSync(fullPath);
        if (stat.size > MARKDOWN_FILE_MAX_BYTES) continue;
        files.push({
          path: fullPath,
          relativePath: normalizeVaultRelativePath(rootPath, fullPath),
          name: entry.name,
          content: readFileSync(fullPath, "utf8"),
        });
        continue;
      }

      if (MARKDOWN_IMAGE_EXTENSIONS.has(ext)) {
        if (images.length >= MARKDOWN_VAULT_MAX_IMAGES) continue;
        const stat = statSync(fullPath);
        const dimensions = readImageDimensions(fullPath);
        images.push({
          path: fullPath,
          relativePath: normalizeVaultRelativePath(rootPath, fullPath),
          name: entry.name,
          fileUrl: localImageUrl(fullPath),
          mimeType: localImageMimeType(fullPath),
          sizeBytes: stat.size,
          width: dimensions?.width ?? null,
          height: dimensions?.height ?? null,
        });
      }
    }
  };

  visit(rootPath);

  return {
    rootPath,
    files,
    images,
  };
}

export function safeVaultOutputPath(rootPath: string, relativePath: string): string {
  const normalizedRelative = relativePath.replace(/\\/g, "/");
  if (
    normalizedRelative.startsWith("/") ||
    normalizedRelative.includes("../") ||
    normalizedRelative === ".." ||
    /^[a-zA-Z]:/.test(normalizedRelative)
  ) {
    throw new Error("[kepler-shell] unsafe Markdown export relative path");
  }

  const outputPath = path.resolve(rootPath, normalizedRelative);
  const root = path.resolve(rootPath);
  if (outputPath !== root && !outputPath.startsWith(`${root}${path.sep}`)) {
    throw new Error("[kepler-shell] Markdown export path escapes output directory");
  }
  return outputPath;
}

export function resolveMarkdownVaultSourcePath(sourcePath: string): string | null {
  const trimmed = sourcePath.trim();
  if (!trimmed) return null;

  if (/^file:/i.test(trimmed)) {
    try {
      const resolved = fileURLToPath(trimmed);
      return path.isAbsolute(resolved) ? resolved : null;
    } catch {
      return null;
    }
  }

  const localImagePath = parseLocalImageRequestUrl(trimmed);
  if (localImagePath) {
    return path.isAbsolute(localImagePath) ? localImagePath : null;
  }

  if (path.isAbsolute(trimmed)) {
    return path.resolve(trimmed);
  }

  return null;
}
