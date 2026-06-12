import { buildEntryMarkdownDocument, type FrontmatterValue } from "./markdownFrontmatter";
import type { NoteType } from "./typedNotes";

export interface ObsidianVaultMarkdownFile {
  path: string;
  relativePath: string;
  name: string;
  content: string;
}

export interface ObsidianVaultImageFile {
  path: string;
  relativePath: string;
  name: string;
  fileUrl: string;
  mimeType: string;
  sizeBytes: number;
  width: number | null;
  height: number | null;
}

export interface ObsidianImportDraft {
  sourcePath: string;
  relativePath: string;
  id?: string;
  title: string;
  typeId: string;
  bodyMarkdown: string;
  headerProps: Record<string, unknown>;
  wikilinks: string[];
  imageRefs: string[];
}

export interface ObsidianImageObjectDraft {
  id: string;
  title: string;
  typeId: string;
  contentJson: object;
  headerProps: Record<string, unknown>;
}

export interface ImportObsidianVaultArgs {
  files: readonly ObsidianVaultMarkdownFile[];
  images?: readonly ObsidianVaultImageFile[];
  noteTypes: readonly NoteType[];
  defaultTypeId: string;
  imageTypeId: string;
}

export interface ImportObsidianVaultResult {
  entries: ObsidianImportDraft[];
  images: ObsidianImageObjectDraft[];
}

export interface BuildObsidianExportFilesArgs {
  entries: readonly Entry[];
  noteTypes: readonly NoteType[];
  bodyMarkdownById: (entry: Entry) => string;
  relatedEntryTitleLookup?: (entryId: string) => string | null | undefined;
}

export interface ObsidianExportFile {
  relativePath: string;
  content: string;
}

type LooseFrontmatter = Record<string, FrontmatterValue | undefined>;

const RESERVED_FRONTMATTER_KEYS = new Set(["eden", "title", "type", "links"]);
const IMAGE_MARKDOWN_RE =
  /!\[([^\]]*)\]\(([^)\s]+)(?:\s+"[^"]*")?\)|!\[\[([^|\]]+)(?:\|([^\]]+))?\]\]/g;
const WIKILINK_RE = /(?<!!)\[\[([^|\]]+)(?:\|([^\]]+))?\]\]/g;

export function importObsidianVault(args: ImportObsidianVaultArgs): ImportObsidianVaultResult {
  const imageAssets = buildImageAssetMap(args.images ?? []);
  const entries = args.files.map((file) =>
    parseObsidianFile(file, args.noteTypes, args.defaultTypeId, imageAssets),
  );
  const images = (args.images ?? []).map((image) => buildImageObjectDraft(image, args.imageTypeId));

  return { entries, images };
}

export function createObsidianVaultImportDrafts(args: {
  files: readonly ObsidianVaultMarkdownFile[];
  noteTypes: readonly NoteType[];
  defaultNoteTypeId: string;
}): Array<
  ObsidianImportDraft & {
    name: string;
    entryId?: string;
    titleReferences: string[];
    imageReferences: string[];
  }
> {
  return importObsidianVault({
    files: args.files,
    noteTypes: args.noteTypes,
    defaultTypeId: args.defaultNoteTypeId,
    imageTypeId: "image_obj",
  }).entries.map((entry) => ({
    ...entry,
    name: entry.relativePath.split("/").pop() ?? entry.relativePath,
    entryId: entry.id,
    titleReferences: entry.wikilinks,
    imageReferences: entry.imageRefs,
  }));
}

export function buildObsidianExportFiles(args: BuildObsidianExportFilesArgs): ObsidianExportFile[] {
  const noteTypesById = new Map(args.noteTypes.map((noteType) => [noteType.id, noteType]));
  const usedPaths = new Set<string>();

  return args.entries
    .filter((entry) => !entry.deleted_at)
    .map((entry) => {
      const baseName = safeMarkdownBaseName(entry.title || entry.id);
      let relativePath = `${baseName}.md`;
      let suffix = 2;
      while (usedPaths.has(relativePath.toLocaleLowerCase("ru"))) {
        relativePath = `${baseName}-${suffix}.md`;
        suffix += 1;
      }
      usedPaths.add(relativePath.toLocaleLowerCase("ru"));

      return {
        relativePath,
        content: buildEntryMarkdownDocument({
          entry,
          noteType: entry.type_id ? noteTypesById.get(entry.type_id) : null,
          bodyMarkdown: args.bodyMarkdownById(entry),
          relatedEntryTitleLookup: args.relatedEntryTitleLookup,
        }),
      };
    });
}

export function exportObsidianVaultMarkdownFiles(args: {
  entries: Array<
    Partial<Entry> & {
      id: string;
      title: string;
      type_id: string | null;
      header_props_json: string;
      schema_version: number;
    }
  >;
  noteTypes: readonly NoteType[];
  bodyMarkdownLookup: Map<string, string>;
  titleLookup: Map<string, string>;
}): ObsidianExportFile[] {
  return buildObsidianExportFiles({
    entries: args.entries.map((entry) => ({
      content_json: "{}",
      created_at: 0,
      updated_at: 0,
      folder_id: null,
      header_layout: "inline",
      deleted_at: null,
      ...entry,
    })) as Entry[],
    noteTypes: args.noteTypes,
    bodyMarkdownById: (entry) => args.bodyMarkdownLookup.get(entry.id) ?? "",
    relatedEntryTitleLookup: (entryId) => args.titleLookup.get(entryId),
  });
}

function parseObsidianFile(
  file: ObsidianVaultMarkdownFile,
  noteTypes: readonly NoteType[],
  defaultTypeId: string,
  imageAssets: Map<string, ObsidianVaultImageFile>,
): ObsidianImportDraft {
  const split = splitLooseFrontmatter(file.content);
  const title = frontmatterString(split.frontmatter.title) || titleFromPath(file.relativePath);
  const eden = isPlainObject(split.frontmatter.eden) ? split.frontmatter.eden : null;
  const id = frontmatterString(eden?.id) || undefined;
  const typeId = resolveTypeId(split.frontmatter, noteTypes) ?? defaultTypeId;
  const bodyMarkdown = rewriteImageReferences(split.bodyMarkdown, file.relativePath, imageAssets);
  const wikilinks = uniqueStrings([
    ...extractBodyWikilinks(bodyMarkdown),
    ...extractFrontmatterWikilinks(split.frontmatter),
  ]);
  const imageRefs = uniqueStrings(extractMarkdownImageRefs(bodyMarkdown));

  return {
    sourcePath: file.path,
    relativePath: file.relativePath,
    id,
    title,
    typeId,
    bodyMarkdown,
    headerProps: extractHeaderProps(split.frontmatter),
    wikilinks,
    imageRefs,
  };
}

function buildImageObjectDraft(
  image: ObsidianVaultImageFile,
  imageTypeId: string,
): ObsidianImageObjectDraft {
  const width = image.width ?? "";
  const height = image.height ?? "";
  const resolution = image.width && image.height ? `${image.width}x${image.height}` : "";

  return {
    id: `image:${stableIdFromPath(image.relativePath)}`,
    title: image.name,
    typeId: imageTypeId,
    contentJson: { type: "doc", content: [{ type: "paragraph" }] },
    headerProps: {
      image: image.fileUrl,
      file_name: image.name,
      mime_type: image.mimeType,
      size_bytes: image.sizeBytes,
      width,
      height,
      resolution,
      source_path: image.path,
      alt_text: titleFromPath(image.relativePath),
    },
  };
}

function splitLooseFrontmatter(markdown: string): {
  frontmatter: LooseFrontmatter;
  bodyMarkdown: string;
} {
  const normalized = markdown.replace(/\r\n?/g, "\n");
  const lines = normalized.split("\n");
  if (lines[0] !== "---") {
    return { frontmatter: {}, bodyMarkdown: normalized };
  }

  const closeIndex = lines.findIndex((line, index) => index > 0 && line === "---");
  if (closeIndex === -1) {
    return { frontmatter: {}, bodyMarkdown: normalized };
  }

  return {
    frontmatter: parseLooseYaml(lines.slice(1, closeIndex)),
    bodyMarkdown: lines.slice(closeIndex + 1).join("\n"),
  };
}

function parseLooseYaml(lines: string[]): LooseFrontmatter {
  const result: LooseFrontmatter = {};
  let currentArrayKey: string | null = null;
  let currentObjectKey: string | null = null;
  let currentNestedArrayKey: string | null = null;

  for (const raw of lines) {
    const indent = raw.match(/^ */)?.[0].length ?? 0;
    const line = raw.trim();
    if (!line || line.startsWith("#")) continue;

    if (currentObjectKey && currentNestedArrayKey && indent >= 4 && line.startsWith("- ")) {
      const obj = result[currentObjectKey];
      if (isPlainObject(obj)) {
        const current = obj[currentNestedArrayKey];
        const next = Array.isArray(current) ? current : [];
        next.push(parseLooseScalar(line.slice(2).trim()));
        obj[currentNestedArrayKey] = next;
      }
      continue;
    }

    if (currentObjectKey && indent >= 2 && !line.startsWith("- ")) {
      const obj = result[currentObjectKey];
      if (!isPlainObject(obj)) continue;
      const index = line.indexOf(":");
      if (index <= 0) continue;
      const key = line.slice(0, index).trim();
      const value = line.slice(index + 1).trim();
      if (!value) {
        obj[key] = [];
        currentNestedArrayKey = key;
      } else {
        obj[key] = parseLooseScalar(value);
        currentNestedArrayKey = null;
      }
      continue;
    }

    if (currentArrayKey && line.startsWith("- ")) {
      const current = result[currentArrayKey];
      const next = Array.isArray(current) ? current : [];
      next.push(parseLooseScalar(line.slice(2).trim()));
      result[currentArrayKey] = next;
      continue;
    }

    currentArrayKey = null;
    currentObjectKey = null;
    currentNestedArrayKey = null;
    const index = line.indexOf(":");
    if (index <= 0) continue;
    const key = line.slice(0, index).trim();
    const value = line.slice(index + 1).trim();
    if (!value) {
      result[key] = {};
      currentObjectKey = key;
      currentArrayKey = key;
    } else {
      result[key] = parseLooseScalar(value);
    }
  }

  return result;
}

function parseLooseScalar(value: string): FrontmatterValue {
  const trimmed = value.trim();
  if (trimmed === "null" || trimmed === "~") return null;
  if (trimmed === "true") return true;
  if (trimmed === "false") return false;
  if (/^-?\d+(\.\d+)?$/.test(trimmed)) return Number(trimmed);
  if (
    (trimmed.startsWith('"') && trimmed.endsWith('"')) ||
    (trimmed.startsWith("'") && trimmed.endsWith("'"))
  ) {
    return trimmed.slice(1, -1);
  }
  return trimmed;
}

function resolveTypeId(
  frontmatter: LooseFrontmatter,
  noteTypes: readonly NoteType[],
): string | null {
  const eden = isPlainObject(frontmatter.eden) ? frontmatter.eden : null;
  const candidates = [frontmatterString(eden?.type), frontmatterString(frontmatter.type)].filter(
    Boolean,
  );

  for (const candidate of candidates) {
    const matched = noteTypes.find(
      (noteType) =>
        noteType.id === candidate || noteType.slug === candidate || noteType.name === candidate,
    );
    if (matched) return matched.id;
  }

  return null;
}

function extractHeaderProps(frontmatter: LooseFrontmatter): Record<string, unknown> {
  const props: Record<string, unknown> = {};
  for (const [key, value] of Object.entries(frontmatter)) {
    if (RESERVED_FRONTMATTER_KEYS.has(key) || key.startsWith("__") || value === undefined) {
      continue;
    }
    props[key] = value;
  }
  return props;
}

function extractFrontmatterWikilinks(value: FrontmatterValue | undefined): string[] {
  if (value === undefined) return [];

  const wikilinks: string[] = [];
  const visit = (current: FrontmatterValue | undefined): void => {
    if (current === undefined) {
      return;
    }

    if (typeof current === "string") {
      wikilinks.push(...extractBodyWikilinks(current));
      return;
    }

    if (Array.isArray(current)) {
      for (const item of current) {
        visit(item);
      }
      return;
    }

    if (isPlainObject(current)) {
      for (const nested of Object.values(current)) {
        visit(nested);
      }
    }
  };

  visit(value);
  return wikilinks;
}

function extractBodyWikilinks(markdown: string): string[] {
  return [...markdown.matchAll(WIKILINK_RE)]
    .map((match) => match[1]?.trim())
    .filter((value): value is string => Boolean(value));
}

function extractMarkdownImageRefs(markdown: string): string[] {
  return [...markdown.matchAll(IMAGE_MARKDOWN_RE)]
    .map((match) => (match[2] ?? match[3] ?? "").trim())
    .filter(Boolean);
}

function rewriteImageReferences(
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
      const src = (inlineSrc ?? wikiSrc ?? "").trim();
      const asset = resolveImageAsset(src, sourceDir, imageAssets);
      if (!asset) return raw;
      const label = (alt || titleFromPath(asset.relativePath)).trim();
      return `![${label}](${asset.fileUrl})`;
    },
  );
}

function buildImageAssetMap(
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

function frontmatterString(value: unknown): string {
  return typeof value === "string" ? value.trim() : "";
}

function titleFromPath(relativePath: string): string {
  const fileName = normalizePath(relativePath).split("/").pop() ?? relativePath;
  return fileName.replace(/\.[^.]+$/, "").trim() || "Без названия";
}

function normalizePath(value: string): string {
  return value.replace(/\\/g, "/").replace(/^\/+/, "").replace(/\/+/g, "/");
}

function safeMarkdownBaseName(value: string): string {
  const safe = value
    .trim()
    .replace(/[<>:"/\\|?*]/g, "-")
    .replace(/./g, (char) => (char.charCodeAt(0) < 32 ? "-" : char))
    .replace(/\s+/g, "-")
    .replace(/-+/g, "-")
    .replace(/^[.\s-]+|[.\s-]+$/g, "");
  return safe || "eden-object";
}

function stableIdFromPath(value: string): string {
  let hash = 0x811c9dc5;
  for (const char of normalizePath(value)) {
    hash ^= char.charCodeAt(0);
    hash = Math.imul(hash, 0x01000193);
  }
  return hash.toString(16).padStart(8, "0");
}

function uniqueStrings(values: string[]): string[] {
  const seen = new Set<string>();
  return values.filter((value) => {
    const trimmed = value.trim();
    if (!trimmed || seen.has(trimmed)) return false;
    seen.add(trimmed);
    return true;
  });
}

function isPlainObject(value: unknown): value is Record<string, FrontmatterValue | undefined> {
  return Boolean(value) && typeof value === "object" && !Array.isArray(value);
}
