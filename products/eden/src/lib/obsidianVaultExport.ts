import { buildEntryMarkdownDocument } from "./markdownFrontmatter";
import { safeVaultPathSegment } from "./obsidianVaultExportAssetPaths";
import {
  rewriteObsidianExportHeaderImageReferences,
  rewriteObsidianExportImageReferences,
  type ObsidianVaultAssetManifestEntry,
} from "./obsidianVaultExportAssets";
import type { NoteType } from "./typedNotes";

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

export interface ObsidianExportFile {
  relativePath: string;
  content?: string;
  sourcePath?: string;
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
