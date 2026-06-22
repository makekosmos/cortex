import type { NoteType } from "./typedNotes";

type FrontmatterScalar = string | number | boolean | null;
export type FrontmatterValue =
  | FrontmatterScalar
  | FrontmatterValue[]
  | { [key: string]: FrontmatterValue };

export interface EntryMarkdownFrontmatter {
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

export interface ParseEntryMarkdownDocumentOptions {
  noteType?: NoteType | null;
  noteTypes?: readonly NoteType[];
}

export interface ParsedEntryMarkdownDocument {
  frontmatter: EntryMarkdownFrontmatter;
  bodyMarkdown: string;
  entryPatch: {
    id?: string;
    title: string;
    typeId?: string;
    headerProps: Record<string, unknown>;
  };
}

interface YamlLine {
  number: number;
  raw: string;
}

interface ParsedWikilink {
  target: string;
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

export function parseEntryMarkdownDocument(
  markdown: string,
  options: ParseEntryMarkdownDocumentOptions = {},
): ParsedEntryMarkdownDocument {
  const { frontmatterSource, bodyMarkdown } = splitMarkdownFrontmatter(markdown);
  const parsed = parseYamlDocument(frontmatterSource);
  const frontmatter = asFrontmatterObject(parsed);
  const headerProps = extractHeaderProps(frontmatter);
  const relatedLinks = extractRelatedLinks(frontmatter.links);

  if (relatedLinks.length > 0) {
    headerProps.related_notes = relatedLinks;
  }

  const title = typeof frontmatter.title === "string" ? frontmatter.title.trim() : "";
  if (!title) {
    throw new Error('Markdown frontmatter must include a non-empty "title"');
  }

  const typeId = resolveTypeId(frontmatter, options);
  const eden = isPlainObject(frontmatter.eden) ? frontmatter.eden : null;
  const entryId = typeof eden?.id === "string" && eden.id.trim() ? eden.id.trim() : undefined;

  return {
    frontmatter,
    bodyMarkdown,
    entryPatch: {
      id: entryId,
      title,
      typeId: typeId ?? undefined,
      headerProps,
    },
  };
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
    throw new Error(`Invalid entry.header_props_json: ${message}`);
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
    return undefined;
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

function splitMarkdownFrontmatter(markdown: string): {
  frontmatterSource: string;
  bodyMarkdown: string;
} {
  const normalized = markdown.replace(/\r\n?/g, "\n");
  const lines = normalized.split("\n");

  if (lines[0] !== FRONTMATTER_BOUNDARY) {
    throw new Error('Markdown document must start with YAML frontmatter fenced by "---"');
  }

  let closingLineIndex = -1;
  for (let index = 1; index < lines.length; index += 1) {
    if (lines[index] === FRONTMATTER_BOUNDARY) {
      closingLineIndex = index;
      break;
    }
  }

  if (closingLineIndex === -1) {
    throw new Error('Markdown frontmatter is missing a closing "---" fence');
  }

  const frontmatterSource = lines.slice(1, closingLineIndex).join("\n");
  const bodyMarkdown = lines.slice(closingLineIndex + 1).join("\n");

  return {
    frontmatterSource,
    bodyMarkdown,
  };
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

function parseYamlDocument(source: string): FrontmatterValue {
  const lines = source.split("\n").map((raw, index) => ({ raw, number: index + 1 }));
  const parser = new YamlSubsetParser(lines);
  return parser.parse();
}

class YamlSubsetParser {
  private readonly lines: YamlLine[];
  private index = 0;

  constructor(lines: YamlLine[]) {
    this.lines = lines;
  }

  parse(): FrontmatterValue {
    this.skipIgnorableLines();

    if (this.index >= this.lines.length) {
      return {};
    }

    const first = this.lines[this.index];
    const indent = countIndent(first.raw, first.number);
    if (indent !== 0) {
      this.error(first.number, "Top-level YAML indentation must start at 0 spaces");
    }

    const value = this.parseBlock(0);
    this.skipIgnorableLines();

    if (this.index < this.lines.length) {
      const line = this.lines[this.index];
      this.error(line.number, "Unexpected trailing YAML content");
    }

    return value;
  }

  private parseBlock(indent: number): FrontmatterValue {
    this.skipIgnorableLines();
    const current = this.peekSignificantLine();
    if (!current || countIndent(current.raw, current.number) < indent) {
      return {};
    }

    const trimmed = current.raw.trimStart();
    if (trimmed.startsWith("- ")) {
      return this.parseSequence(indent);
    }
    if (trimmed === "-") {
      return this.parseSequence(indent);
    }
    return this.parseMapping(indent);
  }

  private parseMapping(indent: number): Record<string, FrontmatterValue> {
    const result: Record<string, FrontmatterValue> = {};

    while (true) {
      this.skipIgnorableLines();
      const line = this.peekSignificantLine();
      if (!line) {
        break;
      }

      const currentIndent = countIndent(line.raw, line.number);
      if (currentIndent < indent) {
        break;
      }
      if (currentIndent > indent) {
        this.error(line.number, `Expected ${indent} spaces of indentation`);
      }

      const trimmed = line.raw.trimStart();
      if (trimmed.startsWith("-")) {
        this.error(line.number, "Unexpected list item where a mapping entry was expected");
      }

      const { key, rest } = splitYamlKeyValue(trimmed, line.number);
      this.index += 1;

      if (!rest.length) {
        const child = this.peekSignificantLine();
        if (!child || countIndent(child.raw, child.number) <= indent) {
          result[key] = {};
          continue;
        }
        if (countIndent(child.raw, child.number) !== indent + 2) {
          this.error(
            child.number,
            `Nested YAML block must be indented by exactly ${indent + 2} spaces`,
          );
        }
        result[key] = this.parseBlock(indent + 2);
        continue;
      }

      result[key] = parseInlineYamlValue(rest, line.number);
    }

    return result;
  }

  private parseSequence(indent: number): FrontmatterValue[] {
    const result: FrontmatterValue[] = [];

    while (true) {
      this.skipIgnorableLines();
      const line = this.peekSignificantLine();
      if (!line) {
        break;
      }

      const currentIndent = countIndent(line.raw, line.number);
      if (currentIndent < indent) {
        break;
      }
      if (currentIndent > indent) {
        this.error(line.number, `Expected ${indent} spaces of indentation`);
      }

      const trimmed = line.raw.trimStart();
      if (!trimmed.startsWith("-")) {
        break;
      }

      if (trimmed === "-") {
        this.index += 1;
        const child = this.peekSignificantLine();
        if (!child || countIndent(child.raw, child.number) <= indent) {
          result.push({});
          continue;
        }
        if (countIndent(child.raw, child.number) !== indent + 2) {
          this.error(
            child.number,
            `Nested YAML block must be indented by exactly ${indent + 2} spaces`,
          );
        }
        result.push(this.parseBlock(indent + 2));
        continue;
      }

      if (!trimmed.startsWith("- ")) {
        this.error(line.number, 'Unsupported YAML list item; expected "- "');
      }

      const rest = trimmed.slice(2);
      if (looksLikeInlineMapping(rest)) {
        this.error(
          line.number,
          "Unsupported YAML array item object syntax; use a nested block under '-' instead",
        );
      }

      this.index += 1;
      result.push(parseInlineYamlValue(rest, line.number));
    }

    return result;
  }

  private peekSignificantLine(): YamlLine | null {
    let cursor = this.index;
    while (cursor < this.lines.length) {
      const line = this.lines[cursor];
      const trimmed = line.raw.trim();
      if (trimmed && !trimmed.startsWith("#")) {
        return line;
      }
      cursor += 1;
    }
    return null;
  }

  private skipIgnorableLines(): void {
    while (this.index < this.lines.length) {
      const trimmed = this.lines[this.index].raw.trim();
      if (trimmed && !trimmed.startsWith("#")) {
        break;
      }
      this.index += 1;
    }
  }

  private error(lineNumber: number, message: string): never {
    throw new Error(`Unsupported YAML frontmatter at line ${lineNumber}: ${message}`);
  }
}

function parseInlineYamlValue(value: string, lineNumber: number): FrontmatterValue {
  const trimmed = value.trim();

  if (trimmed === "{}") {
    return {};
  }
  if (trimmed === "[]") {
    return [];
  }
  if (trimmed.startsWith("{") || trimmed.startsWith("[")) {
    throw new Error(
      `Unsupported YAML frontmatter at line ${lineNumber}: inline collections are not supported`,
    );
  }

  if (trimmed === "null" || trimmed === "~") {
    return null;
  }
  if (trimmed === "true") {
    return true;
  }
  if (trimmed === "false") {
    return false;
  }
  if (/^-?\d+(\.\d+)?$/.test(trimmed)) {
    const parsed = Number(trimmed);
    if (Number.isFinite(parsed)) {
      return parsed;
    }
  }
  if (trimmed.startsWith('"')) {
    if (!trimmed.endsWith('"') || trimmed.length === 1) {
      throw new Error(
        `Unsupported YAML frontmatter at line ${lineNumber}: unterminated double-quoted string`,
      );
    }
    try {
      return JSON.parse(trimmed);
    } catch (error) {
      const message = error instanceof Error ? error.message : String(error);
      throw new Error(
        `Unsupported YAML frontmatter at line ${lineNumber}: invalid double-quoted string (${message})`,
      );
    }
  }
  if (trimmed.startsWith("'")) {
    if (!trimmed.endsWith("'") || trimmed.length === 1) {
      throw new Error(
        `Unsupported YAML frontmatter at line ${lineNumber}: unterminated single-quoted string`,
      );
    }
    return trimmed.slice(1, -1).replace(/''/g, "'");
  }

  return trimmed;
}

function splitYamlKeyValue(line: string, lineNumber: number): { key: string; rest: string } {
  let quote: '"' | "'" | null = null;

  for (let index = 0; index < line.length; index += 1) {
    const char = line[index];
    if (quote) {
      if (char === quote) {
        quote = null;
      }
      continue;
    }
    if (char === '"' || char === "'") {
      quote = char;
      continue;
    }
    if (char === ":") {
      const key = line.slice(0, index).trim();
      const rest = line.slice(index + 1).trim();
      if (!key) {
        throw new Error(
          `Unsupported YAML frontmatter at line ${lineNumber}: mapping key must not be empty`,
        );
      }
      return {
        key: unquoteYamlKey(key, lineNumber),
        rest,
      };
    }
  }

  throw new Error(
    `Unsupported YAML frontmatter at line ${lineNumber}: expected "key: value" mapping syntax`,
  );
}

function unquoteYamlKey(key: string, lineNumber: number): string {
  if (key.startsWith('"')) {
    if (!key.endsWith('"')) {
      throw new Error(
        `Unsupported YAML frontmatter at line ${lineNumber}: unterminated quoted key`,
      );
    }
    try {
      const parsed = JSON.parse(key);
      if (typeof parsed === "string") return parsed;
    } catch (error) {
      const message = error instanceof Error ? error.message : String(error);
      throw new Error(
        `Unsupported YAML frontmatter at line ${lineNumber}: invalid quoted key (${message})`,
      );
    }
    throw new Error(
      `Unsupported YAML frontmatter at line ${lineNumber}: quoted key must be a string`,
    );
  }
  if (key.startsWith("'")) {
    if (!key.endsWith("'")) {
      throw new Error(
        `Unsupported YAML frontmatter at line ${lineNumber}: unterminated quoted key`,
      );
    }
    return key.slice(1, -1).replace(/''/g, "'");
  }
  return key;
}

function looksLikeInlineMapping(value: string): boolean {
  let quote: '"' | "'" | null = null;
  for (let index = 0; index < value.length; index += 1) {
    const char = value[index];
    if (quote) {
      if (char === quote) {
        quote = null;
      }
      continue;
    }
    if (char === '"' || char === "'") {
      quote = char;
      continue;
    }
    if (char === ":") {
      return true;
    }
  }
  return false;
}

function countIndent(raw: string, lineNumber: number): number {
  if (raw.includes("\t")) {
    throw new Error(
      `Unsupported YAML frontmatter at line ${lineNumber}: tabs are not supported for indentation`,
    );
  }

  const match = raw.match(/^ */);
  const indent = match?.[0].length ?? 0;
  if (indent % 2 !== 0) {
    throw new Error(
      `Unsupported YAML frontmatter at line ${lineNumber}: indentation must use 2-space steps`,
    );
  }

  return indent;
}

function asFrontmatterObject(value: FrontmatterValue): EntryMarkdownFrontmatter {
  if (!isPlainObject(value)) {
    throw new Error("Markdown frontmatter root must be a YAML mapping");
  }

  return value as EntryMarkdownFrontmatter;
}

function extractHeaderProps(frontmatter: EntryMarkdownFrontmatter): Record<string, unknown> {
  const headerProps: Record<string, unknown> = {};

  for (const [key, value] of Object.entries(frontmatter)) {
    if (RESERVED_FRONTMATTER_KEYS.has(key) || isInternalHeaderPropKey(key) || value === undefined) {
      continue;
    }
    headerProps[key] = value;
  }

  return headerProps;
}

function extractRelatedLinks(links: FrontmatterValue | undefined): string[] {
  if (!isPlainObject(links)) {
    return [];
  }

  const related = links.related;
  if (!Array.isArray(related)) {
    return [];
  }

  return related
    .map((item) => {
      if (typeof item !== "string") {
        throw new Error('Markdown frontmatter "links.related" must be an array of strings');
      }

      const wikilink = parseWikilink(item);
      return wikilink ? wikilink.target : item.trim();
    })
    .filter(Boolean);
}

function parseWikilink(value: string): ParsedWikilink | null {
  const trimmed = value.trim();
  const match = /^\[\[([^|\]]+)(?:\|([^\]]+))?\]\]$/.exec(trimmed);
  if (!match) {
    return null;
  }

  return {
    target: match[1].trim(),
  };
}

function resolveTypeId(
  frontmatter: EntryMarkdownFrontmatter,
  options: ParseEntryMarkdownDocumentOptions,
): string | null {
  const label = typeof frontmatter.type === "string" ? frontmatter.type.trim() : "";
  const eden = isPlainObject(frontmatter.eden) ? frontmatter.eden : null;
  const canonicalType = typeof eden?.type === "string" ? eden.type.trim() : "";
  const noteTypes = [
    ...(options.noteType ? [options.noteType] : []),
    ...(options.noteTypes ?? []),
  ].filter(
    (noteType, index, all) => all.findIndex((candidate) => candidate.id === noteType.id) === index,
  );

  const matches = (noteType: NoteType, value: string) =>
    noteType.id === value || noteType.slug === value || noteType.name === value;

  if (label) {
    const matched = noteTypes.find((noteType) => matches(noteType, label));
    if (matched) {
      return matched.id;
    }
  }

  if (canonicalType) {
    const matched = noteTypes.find((noteType) => matches(noteType, canonicalType));
    if (matched) {
      return matched.id;
    }
    return canonicalType;
  }

  if (label) {
    return label;
  }

  return null;
}

function isInternalHeaderPropKey(key: string): boolean {
  return key === "internal" || key.startsWith("__");
}

function isPlainObject(value: unknown): value is Record<string, unknown> {
  return Boolean(value) && typeof value === "object" && !Array.isArray(value);
}
