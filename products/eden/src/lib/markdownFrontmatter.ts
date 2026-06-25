import type { NoteType } from "./typedNotes";

type FrontmatterScalar = string | number | boolean | null;
export type FrontmatterValue =
  | FrontmatterScalar
  | FrontmatterValue[]
  | { [key: string]: FrontmatterValue };

interface EntryMarkdownFrontmatter {
  eden?: {
    id?: string;
    type?: string;
    schema_version?: number;
    [key: string]: FrontmatterValue | undefined;
  };
  title?: string;
  type?: string;
  links?: {
    related?: string[];
    [key: string]: FrontmatterValue | undefined;
  };
  [key: string]: FrontmatterValue | undefined;
}

export interface BuildEntryMarkdownDocumentArgs {
  entry: Entry;
  noteType?: NoteType | null;
  bodyMarkdown: string;
  relatedEntryTitleLookup?:
    | ((entryId: string) => string | null | undefined)
    | Record<string, string>;
}

const FRONTMATTER_BOUNDARY = "---";
const RESERVED_FRONTMATTER_KEYS = new Set(["eden", "title", "type", "links"]);

export function buildEntryMarkdownDocument(args: BuildEntryMarkdownDocumentArgs): string {
  const bodyMarkdown = normalizeBodyMarkdown(args.bodyMarkdown);
  const headerProps = parseEntryHeaderProps(args.entry.header_props_json);
  const relatedNotes = normalizeRelatedNotes(headerProps.related_notes);
  const frontmatter = buildFrontmatter(args.entry, args.noteType ?? null, headerProps, {
    relatedNotes,
    relatedEntryTitleLookup: args.relatedEntryTitleLookup,
  });

  return `${FRONTMATTER_BOUNDARY}\n${serializeYamlDocument(frontmatter)}\n${FRONTMATTER_BOUNDARY}\n${bodyMarkdown}`;
}

function buildFrontmatter(
  entry: Entry,
  noteType: NoteType | null,
  headerProps: Record<string, unknown>,
  args: {
    relatedNotes: string[];
    relatedEntryTitleLookup?:
      | ((entryId: string) => string | null | undefined)
      | Record<string, string>;
  },
): EntryMarkdownFrontmatter {
  const frontmatter: EntryMarkdownFrontmatter = {
    eden: {
      id: entry.id,
      type: entry.type_id ?? noteType?.id ?? undefined,
      schema_version: entry.schema_version,
    },
    title: entry.title,
    type: noteType?.slug || entry.type_id || "note_obj",
  };

  for (const [key, value] of Object.entries(headerProps)) {
    if (key === "related_notes" || isInternalHeaderPropKey(key)) {
      continue;
    }

    const normalizedValue = normalizeFrontmatterValue(value, `header_props_json.${key}`);
    if (normalizedValue !== undefined) {
      frontmatter[key] = normalizedValue;
    }
  }

  if (args.relatedNotes.length > 0) {
    frontmatter.links = {
      related: args.relatedNotes.map((relatedId) =>
        buildRelatedWikilink(relatedId, args.relatedEntryTitleLookup),
      ),
    };
  }

  return frontmatter;
}

function parseEntryHeaderProps(raw: string): Record<string, unknown> {
  if (!raw.trim()) {
    return {};
  }

  let parsed: unknown;
  try {
    parsed = JSON.parse(raw);
  } catch (error) {
    const message = error instanceof Error ? error.message : String(error);
    throw new Error(`Invalid entry.header_props_json: ${message}`, { cause: error });
  }

  if (!isPlainObject(parsed)) {
    throw new Error("Invalid entry.header_props_json: expected a JSON object");
  }

  return parsed;
}

function normalizeBodyMarkdown(bodyMarkdown: string): string {
  const normalized = bodyMarkdown.replace(/\r\n?/g, "\n");
  return normalized.startsWith("\n") ? normalized.slice(1) : normalized;
}

function normalizeRelatedNotes(value: unknown): string[] {
  if (!Array.isArray(value)) {
    return [];
  }

  return value
    .filter((item): item is string => typeof item === "string")
    .map((item) => item.trim())
    .filter(Boolean);
}

function buildRelatedWikilink(
  relatedId: string,
  lookup?: ((entryId: string) => string | null | undefined) | Record<string, string>,
): string {
  const resolvedTitle =
    typeof lookup === "function"
      ? lookup(relatedId)
      : lookup && typeof lookup === "object"
        ? lookup[relatedId]
        : null;
  const title = typeof resolvedTitle === "string" ? resolvedTitle.trim() : "";

  return title ? `[[${relatedId}|${title}]]` : `[[${relatedId}]]`;
}

function normalizeFrontmatterValue(value: unknown, path: string): FrontmatterValue | undefined {
  if (value === undefined) {
    return;
  }

  if (
    value === null ||
    typeof value === "string" ||
    typeof value === "number" ||
    typeof value === "boolean"
  ) {
    return value;
  }

  if (Array.isArray(value)) {
    return value.map((item, index) => {
      const normalizedItem = normalizeFrontmatterValue(item, `${path}[${index}]`);
      if (normalizedItem === undefined) {
        throw new Error(`Unsupported undefined value at ${path}[${index}]`);
      }
      return normalizedItem;
    });
  }

  if (isPlainObject(value)) {
    const normalizedObject: Record<string, FrontmatterValue> = {};
    for (const [key, nestedValue] of Object.entries(value)) {
      const normalizedNestedValue = normalizeFrontmatterValue(nestedValue, `${path}.${key}`);
      if (normalizedNestedValue !== undefined) {
        normalizedObject[key] = normalizedNestedValue;
      }
    }
    return normalizedObject;
  }

  throw new Error(`Unsupported header prop value at ${path}`);
}

function serializeYamlDocument(value: { [key: string]: FrontmatterValue | undefined }): string {
  const lines = serializeObjectLines(value, 0);
  return lines.join("\n");
}

function serializeObjectLines(
  value: { [key: string]: FrontmatterValue | undefined },
  indentLevel: number,
): string[] {
  const lines: string[] = [];

  for (const [key, nestedValue] of Object.entries(value)) {
    if (nestedValue === undefined) {
      continue;
    }

    const indent = " ".repeat(indentLevel);
    if (Array.isArray(nestedValue)) {
      if (nestedValue.length === 0) {
        lines.push(`${indent}${key}: []`);
        continue;
      }
      lines.push(`${indent}${key}:`);
      lines.push(...serializeArrayLines(nestedValue, indentLevel + 2));
      continue;
    }

    if (isPlainObject(nestedValue)) {
      const nestedLines = serializeObjectLines(nestedValue, indentLevel + 2);
      if (nestedLines.length === 0) {
        lines.push(`${indent}${key}: {}`);
      } else {
        lines.push(`${indent}${key}:`);
        lines.push(...nestedLines);
      }
      continue;
    }

    lines.push(`${indent}${key}: ${serializeScalar(nestedValue)}`);
  }

  return lines;
}

function serializeArrayLines(value: FrontmatterValue[], indentLevel: number): string[] {
  const lines: string[] = [];
  const indent = " ".repeat(indentLevel);

  for (const item of value) {
    if (Array.isArray(item)) {
      if (item.length === 0) {
        lines.push(`${indent}- []`);
      } else {
        lines.push(`${indent}-`);
        lines.push(...serializeArrayLines(item, indentLevel + 2));
      }
      continue;
    }

    if (isPlainObject(item)) {
      const nestedLines = serializeObjectLines(item, indentLevel + 2);
      if (nestedLines.length === 0) {
        lines.push(`${indent}- {}`);
      } else {
        lines.push(`${indent}-`);
        lines.push(...nestedLines);
      }
      continue;
    }

    lines.push(`${indent}- ${serializeScalar(item)}`);
  }

  return lines;
}

function serializeScalar(value: FrontmatterScalar): string {
  if (value === null) {
    return "null";
  }

  if (typeof value === "number" || typeof value === "boolean") {
    return String(value);
  }

  if (value === "") {
    return '""';
  }

  if (needsQuotedString(value)) {
    return JSON.stringify(value);
  }

  return value;
}

function needsQuotedString(value: string): boolean {
  if (!value.trim() || value !== value.trim()) {
    return true;
  }

  const lowered = value.toLowerCase();
  if (lowered === "null" || lowered === "true" || lowered === "false") {
    return true;
  }

  if (/^-?\d+(\.\d+)?$/.test(value)) {
    return true;
  }

  return !/^[\p{L}\p{N}_./-]+$/u.test(value);
}

function isInternalHeaderPropKey(key: string): boolean {
  return key === "internal" || key.startsWith("__");
}

function isPlainObject(value: unknown): value is Record<string, unknown> {
  return Boolean(value) && typeof value === "object" && !Array.isArray(value);
}
