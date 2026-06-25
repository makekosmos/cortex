import type { FrontmatterValue } from "./markdownFrontmatter";
import type { NoteType } from "./typedNotes";

type LooseFrontmatter = Record<string, FrontmatterValue | undefined>;

const RESERVED_FRONTMATTER_KEYS = new Set(["eden", "title", "type", "links"]);
const WIKILINK_RE = /(?<!!)\[\[([^|\]]+)(?:\|([^\]]+))?\]\]/g;

export function splitLooseFrontmatter(markdown: string): {
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

export function resolveTypeId(
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

export function extractHeaderProps(frontmatter: LooseFrontmatter): Record<string, unknown> {
  const props: Record<string, unknown> = {};
  for (const [key, value] of Object.entries(frontmatter)) {
    if (RESERVED_FRONTMATTER_KEYS.has(key) || key.startsWith("__") || value === undefined) {
      continue;
    }
    props[key] = value;
  }
  return props;
}

export function extractFrontmatterWikilinks(value: FrontmatterValue | undefined): string[] {
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

export function extractBodyWikilinks(markdown: string): string[] {
  return [...markdown.matchAll(WIKILINK_RE)]
    .map((match) => match[1]?.trim())
    .filter((value): value is string => Boolean(value));
}

export function normalizeObsidianTitleTarget(target: string): string {
  return target
    .trim()
    .replace(/\.md$/i, "")
    .replace(/[#^].*$/, "")
    .trim();
}

export function frontmatterString(value: unknown): string {
  return typeof value === "string" ? value.trim() : "";
}

export function isPlainObject(
  value: unknown,
): value is Record<string, FrontmatterValue | undefined> {
  return Boolean(value) && typeof value === "object" && !Array.isArray(value);
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
