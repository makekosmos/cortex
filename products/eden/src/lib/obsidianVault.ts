import { writeEntryMarkdown } from "../editor-cm/content";
import { buildEntryMarkdownDocument, type FrontmatterValue } from "./markdownFrontmatter";
import { parseHeaderTemplate, parseNoteTypeDefinition, type NoteType } from "./typedNotes";

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

export interface ObsidianRelatedImportPlan {
  firstPassHeaderProps: Record<string, unknown>;
  secondPassRelatedIds: string[];
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

export interface ObsidianVaultFolder {
  id: string;
  name: string;
  parent_id: string | null;
}

export interface BuildObsidianExportFilesArgs {
  entries: readonly Entry[];
  noteTypes: readonly NoteType[];
  bodyMarkdownById: (entry: Entry) => string;
  relatedEntryTitleLookup?: (entryId: string) => string | null | undefined;
  selectedTypeIds?: readonly string[];
  folderPathById?: ReadonlyMap<string, string>;
}

export interface ObsidianVaultAssetManifestEntry {
  source: string;
  sourcePath: string | null;
  noteRelativePath: string;
  targetRelativePath: string;
  rewrittenRelativePath: string | null;
  copyable: boolean;
  limitation: string | null;
}

export interface ObsidianExportFile {
  relativePath: string;
  content?: string;
  sourcePath?: string;
}

interface ObsidianExportAssetPlan {
  targetRelativePath: string;
  sourcePath: string | null;
  assetKey: string | null;
}

interface HeaderImageAssetReference {
  source: string;
  preferredFileName?: string | null;
  mimeType?: string | null;
}

type LooseFrontmatter = Record<string, FrontmatterValue | undefined>;

const RESERVED_FRONTMATTER_KEYS = new Set(["eden", "title", "type", "links"]);
const IMAGE_MARKDOWN_RE = /!\[([^\]]*)\]\(([^)\n]+)\)|!\[\[([^|\]]+)(?:\|([^\]]+))?\]\]/g;
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
  images?: readonly ObsidianVaultImageFile[];
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
    images: args.images,
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

export function buildObsidianRelatedImportPlan(args: {
  draft: Pick<ObsidianImportDraft, "headerProps" | "id" | "title" | "wikilinks">;
  entryId: string;
  importedTitleIds: ReadonlyMap<string, string>;
  existingTitleIds?: ReadonlyMap<string, string>;
}): ObsidianRelatedImportPlan {
  const firstPassHeaderProps = { ...args.draft.headerProps };
  delete firstPassHeaderProps.related_notes;

  const secondPassRelatedIds = uniqueStrings(
    args.draft.wikilinks
      .map(normalizeObsidianTitleTarget)
      .filter(Boolean)
      .map((target) => {
        const normalized = target.toLocaleLowerCase("ru");
        return (
          args.importedTitleIds.get(normalized) ?? args.existingTitleIds?.get(normalized) ?? null
        );
      })
      .filter((target): target is string => Boolean(target && target !== args.entryId)),
  );

  return {
    firstPassHeaderProps,
    secondPassRelatedIds,
  };
}

export function buildObsidianExportFiles(args: BuildObsidianExportFilesArgs): ObsidianExportFile[] {
  const noteTypesById = new Map(args.noteTypes.map((noteType) => [noteType.id, noteType]));
  const entriesById = new Map(args.entries.map((entry) => [entry.id, entry]));
  const usedPaths = new Set<string>();
  const selectedTypeIds = args.selectedTypeIds ? new Set(args.selectedTypeIds) : null;
  const assetManifestEntries: ObsidianVaultAssetManifestEntry[] = [];
  const seenAssetManifestKeys = new Set<string>();
  const assetFilesBySource = new Map<string, ObsidianExportFile>();

  const files = args.entries
    .filter((entry) => !entry.deleted_at)
    .filter((entry) => !selectedTypeIds || !entry.type_id || selectedTypeIds.has(entry.type_id))
    .map((entry) => {
      const relativePath = buildObsidianExportRelativePath(entry, args.folderPathById, usedPaths);
      const noteType = entry.type_id ? noteTypesById.get(entry.type_id) : null;
      const rewrittenHeaderPropsJson = rewriteObsidianExportHeaderImageReferences({
        entry,
        noteType,
        noteRelativePath: relativePath,
        entriesById,
        usedPaths,
        assetFilesBySource,
        assetManifestEntries,
        seenAssetManifestKeys,
      });
      const exportEntry = rewrittenHeaderPropsJson
        ? ({ ...entry, header_props_json: rewrittenHeaderPropsJson } as Entry)
        : entry;
      const rewrittenBody = rewriteObsidianExportImageReferences({
        markdown: args.bodyMarkdownById(entry),
        noteRelativePath: relativePath,
        usedPaths,
        assetFilesBySource,
        assetManifestEntries,
        seenAssetManifestKeys,
      });

      return {
        relativePath,
        content: buildEntryMarkdownDocument({
          entry: exportEntry,
          noteType,
          bodyMarkdown: rewrittenBody,
          relatedEntryTitleLookup: args.relatedEntryTitleLookup,
        }),
      } satisfies ObsidianExportFile;
    });

  const assetFiles = [...assetFilesBySource.values()];
  if (assetFiles.length === 0 && assetManifestEntries.length === 0) {
    return files;
  }

  const output: ObsidianExportFile[] = [...files, ...assetFiles];
  if (assetManifestEntries.length > 0) {
    output.push({
      relativePath: "assets/obsidian-asset-manifest.json",
      content: JSON.stringify(
        {
          limitation:
            "Some asset references cannot be resolved to a safe local source path; those references stay unchanged and are recorded here as export limitations.",
          assets: assetManifestEntries,
        },
        null,
        2,
      ),
    });
  }

  return output;
}

export function buildObsidianFolderPathLookup(
  folders: readonly ObsidianVaultFolder[],
): ReadonlyMap<string, string> {
  const folderById = new Map(folders.map((folder) => [folder.id, folder]));
  const pathById = new Map<string, string>();
  const resolving = new Set<string>();

  const resolveFolderPath = (folderId: string): string | null => {
    const cached = pathById.get(folderId);
    if (cached) return cached;

    const folder = folderById.get(folderId);
    if (!folder || resolving.has(folderId)) return null;

    resolving.add(folderId);
    const parentPath = folder.parent_id ? resolveFolderPath(folder.parent_id) : null;
    resolving.delete(folderId);

    const segment = safeVaultPathSegment(folder.name);
    const resolved = parentPath ? `${parentPath}/${segment}` : segment;
    pathById.set(folderId, resolved);
    return resolved;
  };

  for (const folder of folders) {
    resolveFolderPath(folder.id);
  }

  return pathById;
}

function buildObsidianExportRelativePath(
  entry: Pick<Entry, "id" | "title" | "folder_id">,
  folderPathById: ReadonlyMap<string, string> | undefined,
  usedPaths: Set<string>,
): string {
  const baseName = safeMarkdownBaseName(entry.title || entry.id);
  const folderPath = entry.folder_id ? (folderPathById?.get(entry.folder_id) ?? "") : "";

  let relativePath = folderPath ? `${folderPath}/${baseName}.md` : `${baseName}.md`;
  let suffix = 2;
  while (usedPaths.has(relativePath.toLocaleLowerCase("ru"))) {
    const leafName = `${baseName}-${suffix}.md`;
    relativePath = folderPath ? `${folderPath}/${leafName}` : leafName;
    suffix += 1;
  }
  usedPaths.add(relativePath.toLocaleLowerCase("ru"));
  return relativePath;
}

function safeVaultPathSegment(value: string): string {
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
    contentJson: writeEntryMarkdown(`![](${image.fileUrl})`),
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
  const normalized = markdown.replace(/^\uFEFF/, "").replace(/\r\n?/g, "\n");
  const lines = normalized.split("\n");
  if (lines[0]?.trim() !== "---") {
    return { frontmatter: {}, bodyMarkdown: normalized };
  }

  const closeIndex = lines.findIndex((line, index) => {
    const trimmed = line.trim();
    return index > 0 && (trimmed === "---" || trimmed === "...");
  });
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
  if (trimmed.startsWith("[") && trimmed.endsWith("]")) {
    return splitInlineYamlArray(trimmed.slice(1, -1)).map((item) => parseLooseScalar(item));
  }
  if (
    (trimmed.startsWith('"') && trimmed.endsWith('"')) ||
    (trimmed.startsWith("'") && trimmed.endsWith("'"))
  ) {
    return unquoteLooseScalar(trimmed);
  }
  return trimmed;
}

function splitInlineYamlArray(value: string): string[] {
  const items: string[] = [];
  let current = "";
  let quote: '"' | "'" | null = null;
  let escapeNext = false;

  for (const char of value) {
    if (escapeNext) {
      current += char;
      escapeNext = false;
      continue;
    }

    if (char === "\\" && quote === '"') {
      current += char;
      escapeNext = true;
      continue;
    }

    if ((char === '"' || char === "'") && (!quote || quote === char)) {
      quote = quote ? null : char;
      current += char;
      continue;
    }

    if (char === "," && !quote) {
      const item = current.trim();
      if (item) items.push(item);
      current = "";
      continue;
    }

    current += char;
  }

  const tail = current.trim();
  if (tail) items.push(tail);
  return items;
}

function unquoteLooseScalar(value: string): string {
  const quote = value[0];
  const inner = value.slice(1, -1);
  if (quote !== '"') return inner;
  return inner.replace(/\\"/g, '"').replace(/\\\\/g, "\\");
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

function normalizeObsidianTitleTarget(target: string): string {
  return target
    .trim()
    .replace(/\.md$/i, "")
    .replace(/[#^].*$/, "")
    .trim();
}

function extractMarkdownImageRefs(markdown: string): string[] {
  return [...markdown.matchAll(IMAGE_MARKDOWN_RE)]
    .map((match) => parseMarkdownImageTarget(match[2] ?? match[3] ?? ""))
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
      const src = parseMarkdownImageTarget(inlineSrc ?? wikiSrc ?? "");
      const asset = resolveImageAsset(src, sourceDir, imageAssets);
      if (!asset) return raw;
      const label = (alt || titleFromPath(asset.relativePath)).trim();
      return `![${label}](${asset.fileUrl})`;
    },
  );
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

function rewriteObsidianExportHeaderImageReferences(args: {
  entry: Entry;
  noteType: NoteType | null | undefined;
  noteRelativePath: string;
  entriesById: ReadonlyMap<string, Entry>;
  usedPaths: Set<string>;
  assetFilesBySource: Map<string, ObsidianExportFile>;
  assetManifestEntries: ObsidianVaultAssetManifestEntry[];
  seenAssetManifestKeys: Set<string>;
}): string | null {
  const headerProps = parseEntryHeaderPropsJsonLoose(args.entry.header_props_json);
  if (!headerProps) return null;

  const imageFieldIds = exportImageFieldIds(args.entry, args.noteType);
  if (imageFieldIds.size === 0) return null;

  let rewrittenHeaderProps: Record<string, unknown> | null = null;
  for (const fieldId of imageFieldIds) {
    if (!(fieldId in headerProps)) continue;

    const reference = resolveHeaderImageAssetReference({
      fieldId,
      value: headerProps[fieldId],
      headerProps,
      entriesById: args.entriesById,
    });
    if (!reference) continue;

    const registeredAsset = registerObsidianExportAssetReference({
      source: reference.source,
      noteRelativePath: args.noteRelativePath,
      preferredFileName: reference.preferredFileName,
      mimeType: reference.mimeType,
      usedPaths: args.usedPaths,
      assetFilesBySource: args.assetFilesBySource,
      assetManifestEntries: args.assetManifestEntries,
      seenAssetManifestKeys: args.seenAssetManifestKeys,
    });
    if (!registeredAsset) continue;

    rewrittenHeaderProps ??= { ...headerProps };
    rewrittenHeaderProps[fieldId] = rewriteHeaderImageValue(
      headerProps[fieldId],
      registeredAsset.rewrittenRelativePath,
    );
  }

  return rewrittenHeaderProps ? JSON.stringify(rewrittenHeaderProps) : null;
}

function exportImageFieldIds(entry: Entry, noteType: NoteType | null | undefined): Set<string> {
  const fieldIds = new Set<string>();
  if (entry.type_id === "image_obj" || noteType?.slug === "image") {
    fieldIds.add("image");
  }

  if (!noteType) return fieldIds;

  try {
    const headerTemplate = parseHeaderTemplate(noteType.header_template_json);
    if (headerTemplate.imageFieldId) fieldIds.add(headerTemplate.imageFieldId);
  } catch {
    // Ignore malformed legacy templates during best-effort export.
  }

  try {
    for (const field of parseNoteTypeDefinition(noteType.schema_json).fields) {
      if (field.kind === "image") fieldIds.add(field.id);
    }
  } catch {
    // Ignore malformed schemas during best-effort export.
  }

  return fieldIds;
}

function parseEntryHeaderPropsJsonLoose(raw: string): Record<string, unknown> | null {
  try {
    const parsed: unknown = JSON.parse(raw || "{}");
    return isPlainObject(parsed) ? { ...parsed } : null;
  } catch {
    return null;
  }
}

function resolveHeaderImageAssetReference(args: {
  fieldId: string;
  value: unknown;
  headerProps: Record<string, unknown>;
  entriesById: ReadonlyMap<string, Entry>;
}): HeaderImageAssetReference | null {
  const candidate = firstStringValue(args.value);
  const inlineMetadata = extractHeaderImageAssetMetadata(args.value);
  const linkedEntry = candidate ? args.entriesById.get(candidate) : null;
  if (linkedEntry) {
    const linkedProps = parseEntryHeaderPropsJsonLoose(linkedEntry.header_props_json) ?? {};
    const source =
      firstStringValue(linkedProps.source_path) ?? firstStringValue(linkedProps.image) ?? null;
    if (!source) return null;
    return {
      source,
      preferredFileName: firstStringValue(linkedProps.file_name),
      mimeType: firstStringValue(linkedProps.mime_type),
    };
  }

  const metadataSource =
    args.fieldId === "image"
      ? (firstStringValue(args.headerProps.source_path) ?? inlineMetadata?.source ?? null)
      : (inlineMetadata?.source ?? null);
  const source = metadataSource ?? candidate;
  if (!source) return null;

  return {
    source,
    preferredFileName:
      args.fieldId === "image"
        ? (firstStringValue(args.headerProps.file_name) ??
          inlineMetadata?.preferredFileName ??
          null)
        : (inlineMetadata?.preferredFileName ?? null),
    mimeType:
      args.fieldId === "image"
        ? (firstStringValue(args.headerProps.mime_type) ?? inlineMetadata?.mimeType ?? null)
        : (inlineMetadata?.mimeType ?? null),
  };
}

function extractHeaderImageAssetMetadata(value: unknown): HeaderImageAssetReference | null {
  if (!isPlainObject(value)) return null;

  const source = firstStringValue(value.source_path) ?? firstStringValue(value.image);
  if (!source) return null;

  return {
    source,
    preferredFileName: firstStringValue(value.file_name),
    mimeType: firstStringValue(value.mime_type),
  };
}

function firstStringValue(value: unknown): string | null {
  if (typeof value === "string") {
    const trimmed = value.trim();
    return trimmed || null;
  }

  if (Array.isArray(value)) {
    for (const item of value) {
      const candidate = firstStringValue(item);
      if (candidate) return candidate;
    }
  }

  return null;
}

function rewriteHeaderImageValue(value: unknown, rewrittenRelativePath: string): unknown {
  if (Array.isArray(value)) {
    let replaced = false;
    return value.map((item) => {
      if (!replaced && typeof item === "string" && item.trim()) {
        replaced = true;
        return rewrittenRelativePath;
      }
      return item;
    });
  }

  return rewrittenRelativePath;
}

function registerObsidianExportAssetReference(args: {
  source: string;
  noteRelativePath: string;
  preferredFileName?: string | null;
  mimeType?: string | null;
  usedPaths: Set<string>;
  assetFilesBySource: Map<string, ObsidianExportFile>;
  assetManifestEntries: ObsidianVaultAssetManifestEntry[];
  seenAssetManifestKeys: Set<string>;
}): { targetRelativePath: string; rewrittenRelativePath: string; sourcePath: string } | null {
  const noteDir = args.noteRelativePath.includes("/")
    ? args.noteRelativePath.slice(0, args.noteRelativePath.lastIndexOf("/"))
    : "";
  const assetPlan = deriveObsidianExportAssetPlan(args.source, {
    preferredFileName: args.preferredFileName,
    mimeType: args.mimeType,
  });
  if (!assetPlan) return null;

  const copyableSourcePath = assetPlan.sourcePath;
  if (!copyableSourcePath) {
    const manifestKey = `${args.source}\u0000${noteDir}\u0000${assetPlan.targetRelativePath}`;
    if (!args.seenAssetManifestKeys.has(manifestKey)) {
      args.seenAssetManifestKeys.add(manifestKey);
      args.assetManifestEntries.push({
        source: args.source,
        sourcePath: null,
        noteRelativePath: args.noteRelativePath,
        targetRelativePath: assetPlan.targetRelativePath,
        rewrittenRelativePath: null,
        copyable: false,
        limitation:
          "The reference could not be resolved to a safe local source path, so the original Markdown reference was kept unchanged.",
      });
    }
    return null;
  }

  const assetKey = assetPlan.assetKey ?? copyableSourcePath;
  const existingAssetFile = args.assetFilesBySource.get(assetKey);
  const targetRelativePath =
    existingAssetFile?.relativePath ??
    allocateUniqueExportPath(assetPlan.targetRelativePath, args.usedPaths);
  const rewrittenRelativePath = relativePathBetween(noteDir, targetRelativePath);

  if (!existingAssetFile) {
    args.assetFilesBySource.set(assetKey, {
      relativePath: targetRelativePath,
      sourcePath: copyableSourcePath,
    });
  }

  return { targetRelativePath, rewrittenRelativePath, sourcePath: copyableSourcePath };
}

function rewriteObsidianExportImageReferences(args: {
  markdown: string;
  noteRelativePath: string;
  usedPaths: Set<string>;
  assetFilesBySource: Map<string, ObsidianExportFile>;
  assetManifestEntries: ObsidianVaultAssetManifestEntry[];
  seenAssetManifestKeys: Set<string>;
}): string {
  return args.markdown.replace(
    IMAGE_MARKDOWN_RE,
    (raw, alt: string, inlineSrc: string, wikiSrc: string) => {
      const source = parseMarkdownImageTarget(inlineSrc ?? wikiSrc ?? "");
      const registeredAsset = registerObsidianExportAssetReference({
        source,
        noteRelativePath: args.noteRelativePath,
        usedPaths: args.usedPaths,
        assetFilesBySource: args.assetFilesBySource,
        assetManifestEntries: args.assetManifestEntries,
        seenAssetManifestKeys: args.seenAssetManifestKeys,
      });
      if (!registeredAsset) return raw;

      const label = (alt || titleFromPath(registeredAsset.targetRelativePath)).trim();
      return `![${label}](${registeredAsset.rewrittenRelativePath})`;
    },
  );
}

function deriveObsidianExportAssetPlan(
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

function allocateUniqueExportPath(relativePath: string, usedPaths: Set<string>): string {
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

function relativePathBetween(fromDir: string, toPath: string): string {
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
