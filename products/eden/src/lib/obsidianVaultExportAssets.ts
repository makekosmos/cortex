import { parseHeaderTemplate, parseNoteTypeDefinition, type NoteType } from "./typedNotes";
import {
  allocateUniqueExportPath,
  deriveObsidianExportAssetPlan,
  parseMarkdownImageTarget,
  relativePathBetween,
  titleFromPath,
} from "./obsidianVaultExportAssetPaths";
import type { ObsidianExportFile } from "./obsidianVaultExport";

const EMPTY_PROPS: Record<string, never> = Object.freeze({});
const IMAGE_MARKDOWN_RE = /!\[([^\]]*)\]\(([^)\n]+)\)|!\[\[([^|\]]+)(?:\|([^\]]+))?\]\]/g;

export interface ObsidianVaultAssetManifestEntry {
  source: string;
  sourcePath: string | null;
  noteRelativePath: string;
  targetRelativePath: string;
  rewrittenRelativePath: string | null;
  copyable: boolean;
  limitation: string | null;
}

interface HeaderImageAssetReference {
  source: string;
  preferredFileName?: string | null;
  mimeType?: string | null;
}

export function rewriteObsidianExportHeaderImageReferences(args: {
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

export function rewriteObsidianExportImageReferences(args: {
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
    const linkedProps =
      parseEntryHeaderPropsJsonLoose(linkedEntry.header_props_json) ?? EMPTY_PROPS;
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

function isPlainObject(value: unknown): value is Record<string, unknown> {
  return Boolean(value) && typeof value === "object" && !Array.isArray(value);
}
