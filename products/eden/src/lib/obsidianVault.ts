import { writeEntryMarkdown } from "../editor-cm/content";
import {
  buildImageAssetMap,
  extractMarkdownImageRefs,
  rewriteImageReferences,
  stableIdFromPath,
  titleFromPath,
} from "./obsidianVaultImportImages";
import {
  extractBodyWikilinks,
  extractFrontmatterWikilinks,
  extractHeaderProps,
  frontmatterString,
  isPlainObject,
  normalizeObsidianTitleTarget,
  resolveTypeId,
  splitLooseFrontmatter,
} from "./obsidianVaultImportFrontmatter";
import type { NoteType } from "./typedNotes";

export {
  buildObsidianExportFiles,
  buildObsidianFolderPathLookup,
  exportObsidianVaultMarkdownFiles,
} from "./obsidianVaultExport";

const EMPTY_IMAGES = Object.freeze([]) as unknown as ObsidianVaultImageFile[];

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

interface ObsidianImageObjectDraft {
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

export function importObsidianVault(args: ImportObsidianVaultArgs): ImportObsidianVaultResult {
  const sourceImages = args.images ?? EMPTY_IMAGES;
  const imageAssets = buildImageAssetMap(sourceImages);
  const entries = args.files.map((file) =>
    parseObsidianFile(file, args.noteTypes, args.defaultTypeId, imageAssets),
  );
  const images = sourceImages.map((image) => buildImageObjectDraft(image, args.imageTypeId));

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

function uniqueStrings(values: string[]): string[] {
  const seen = new Set<string>();
  return values.filter((value) => {
    const trimmed = value.trim();
    if (!trimmed || seen.has(trimmed)) return false;
    seen.add(trimmed);
    return true;
  });
}
