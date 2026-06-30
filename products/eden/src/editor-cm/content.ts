const MARKDOWN_CONTENT_VERSION = 1;
const TIPTAP_CONTENT_VERSION = 1;

export interface MarkdownContent {
  type: "markdown";
  version: typeof MARKDOWN_CONTENT_VERSION;
  text: string;
}

export interface TiptapContent {
  type: "tiptap";
  version: typeof TIPTAP_CONTENT_VERSION;
  doc: TiptapDoc;
}

export interface TiptapDoc {
  type: "doc";
  content?: TiptapNode[];
}

export type TiptapNode = {
  type?: string;
  text?: string;
  attrs?: Record<string, unknown>;
  marks?: Array<{ type?: string; attrs?: Record<string, unknown> }>;
  content?: TiptapNode[];
};

export function isMarkdownContent(value: unknown): value is MarkdownContent {
  return !!value && typeof value === "object" && (value as { type?: unknown }).type === "markdown";
}

export function isTiptapContent(value: unknown): value is TiptapContent {
  return (
    !!value &&
    typeof value === "object" &&
    (value as { type?: unknown }).type === "tiptap" &&
    isTiptapDoc((value as { doc?: unknown }).doc)
  );
}

export function writeEntryMarkdown(md: string): MarkdownContent {
  return {
    type: "markdown",
    version: MARKDOWN_CONTENT_VERSION,
    text: md,
  };
}

export function writeEntryTiptapDoc(doc: TiptapDoc): TiptapContent {
  return {
    type: "tiptap",
    version: TIPTAP_CONTENT_VERSION,
    doc,
  };
}

export function readEntryMarkdown(value: unknown): string {
  if (typeof value === "string") {
    try {
      return readEntryMarkdown(JSON.parse(value));
    } catch {
      return "";
    }
  }

  if (isMarkdownContent(value)) {
    return typeof value.text === "string" ? value.text : "";
  }

  if (isTiptapContent(value)) {
    return tiptapDocToMarkdown(value.doc);
  }

  if (isTiptapDoc(value)) {
    return tiptapDocToMarkdown(value);
  }

  return legacyProseMirrorToText(value);
}

export function readEntryTiptapDoc(value: unknown): TiptapDoc {
  if (typeof value === "string") {
    try {
      return readEntryTiptapDoc(JSON.parse(value));
    } catch {
      return markdownToTiptapDoc("");
    }
  }

  if (isTiptapContent(value)) return normalizeTiptapDoc(value.doc);
  if (isMarkdownContent(value)) return markdownToTiptapDoc(value.text);
  if (isTiptapDoc(value)) return normalizeTiptapDoc(value);
  return markdownToTiptapDoc(legacyProseMirrorToText(value));
}

export function isEntryTiptapContent(value: unknown): boolean {
  if (typeof value === "string") {
    try {
      return isEntryTiptapContent(JSON.parse(value));
    } catch {
      return false;
    }
  }
  return isTiptapContent(value);
}

export function isReadableEntryContent(value: unknown): boolean {
  if (typeof value === "string") {
    try {
      return isReadableEntryContent(JSON.parse(value));
    } catch {
      return false;
    }
  }

  return (
    isMarkdownContent(value) ||
    isTiptapContent(value) ||
    isTiptapDoc(value) ||
    Boolean(legacyProseMirrorToText(value))
  );
}

export function legacyProseMirrorToText(value: unknown): string {
  try {
    return renderLegacyNode(value).trim();
  } catch {
    return "";
  }
}

type LegacyNode = {
  type?: unknown;
  text?: unknown;
  attrs?: unknown;
  marks?: unknown;
  content?: unknown;
};

type TiptapMark = { type?: string; attrs?: Record<string, unknown> };

function renderLegacyNode(value: unknown): string {
  if (!value || typeof value !== "object") return "";

  const node = value as LegacyNode;
  if (typeof node.text === "string") {
    return renderMarkedText(node.text, normalizeMarks(node.marks));
  }

  const media = mediaMarkdown(node);
  if (media) return media;

  const children = Array.isArray(node.content)
    ? node.content.map(renderLegacyNode).filter(Boolean)
    : [];
  const type = typeof node.type === "string" ? node.type : "";

  switch (type) {
    case "doc":
      return joinBlocks(children);
    case "paragraph":
      return joinInline(children);
    case "heading":
      return renderLegacyHeading(node, children);
    case "blockquote":
      return joinBlocks(children)
        .split("\n")
        .map((line) => (line ? `> ${line}` : ">"))
        .join("\n");
    case "bulletList":
      return renderLegacyList(node, "bullet");
    case "orderedList":
      return renderLegacyList(node, "ordered");
    case "taskList":
      return renderLegacyList(node, "task");
    case "listItem":
    case "taskItem":
      return joinBlocks(children);
    case "codeBlock":
      return renderCodeFence(joinInline(children), node.attrs);
    case "horizontalRule":
      return "---";
    case "hardBreak":
    case "hard_break":
      return "\n";
    default:
      return children.length > 0 ? joinBlocks(children) : "";
  }
}

function renderLegacyHeading(node: LegacyNode, children: string[]): string {
  const attrs =
    node.attrs && typeof node.attrs === "object" ? (node.attrs as Record<string, unknown>) : {};
  const rawLevel = attrs.level;
  const level = typeof rawLevel === "number" ? Math.min(Math.max(rawLevel, 1), 6) : 1;
  return `${"#".repeat(level)} ${joinInline(children)}`.trim();
}

function mediaMarkdown(node: LegacyNode): string {
  const type = typeof node.type === "string" ? node.type.toLowerCase() : "";
  const isMedia = ["image", "media", "imageblock", "figure", "attachment"].some((marker) =>
    type.includes(marker),
  );
  if (!isMedia) return "";

  const attrs =
    node.attrs && typeof node.attrs === "object" ? (node.attrs as Record<string, unknown>) : {};
  for (const key of ["src", "url", "href", "fileUrl"]) {
    const candidate = attrs[key];
    if (typeof candidate === "string" && candidate.trim()) {
      return `![](${candidate})`;
    }
  }
  return "";
}

function joinInline(parts: string[]): string {
  return parts.join("");
}

function joinBlocks(parts: string[]): string {
  return parts.join("\n\n");
}

function renderLegacyList(node: LegacyNode, kind: "bullet" | "ordered" | "task"): string {
  const items = Array.isArray(node.content) ? node.content : [];
  const attrs =
    node.attrs && typeof node.attrs === "object" ? (node.attrs as Record<string, unknown>) : {};
  const orderedStart =
    kind === "ordered" && typeof attrs.start === "number" ? Number(attrs.start) : 1;

  return items
    .map((item, index) => {
      const body = renderLegacyNode(item);
      const prefix =
        kind === "ordered"
          ? `${orderedStart + index}. `
          : kind === "task"
            ? legacyTaskPrefix(item)
            : "- ";
      return prefixMultiline(body, prefix);
    })
    .join("\n");
}

function legacyTaskPrefix(value: unknown): string {
  const attrs =
    value && typeof value === "object" && (value as { attrs?: unknown }).attrs
      ? ((value as { attrs: Record<string, unknown> }).attrs ?? {})
      : {};
  return attrs.checked === true ? "- [x] " : "- [ ] ";
}

function prefixMultiline(text: string, prefix: string): string {
  return text
    .split("\n")
    .map((line, index) => (index === 0 ? `${prefix}${line}` : `  ${line}`))
    .join("\n");
}

function isTiptapDoc(value: unknown): value is TiptapDoc {
  return !!value && typeof value === "object" && (value as { type?: unknown }).type === "doc";
}

function normalizeTiptapDoc(doc: TiptapDoc): TiptapDoc {
  return {
    type: "doc",
    content: Array.isArray(doc.content) ? doc.content : [{ type: "paragraph" }],
  };
}

export function markdownToTiptapDoc(md: string): TiptapDoc {
  const content = parseMarkdownBlocks(md.replace(/\r\n?/g, "\n").split("\n"));
  return {
    type: "doc",
    content: content.length ? content : [{ type: "paragraph" }],
  };
}

export function tiptapDocToMarkdown(doc: TiptapDoc): string {
  return renderTiptapNodes(doc.content ?? []).trim();
}

function renderTiptapNodes(nodes: TiptapNode[]): string {
  const rendered = nodes
    .map((node) => ({ node, text: renderTiptapNode(node) }))
    .filter((entry) => Boolean(entry.text));

  return rendered
    .map((entry, index) => {
      if (index === 0) return entry.text;
      const prev = rendered[index - 1];
      const separator = prev && isListNode(prev.node) && isListNode(entry.node) ? "\n" : "\n\n";
      return `${separator}${entry.text}`;
    })
    .join("");
}

function renderInline(nodes: TiptapNode[] | undefined): string {
  return (nodes ?? []).map(renderInlineNode).join("");
}

function renderTiptapNode(node: TiptapNode): string {
  switch (node.type) {
    case "paragraph":
      return renderInline(node.content);
    case "heading": {
      const rawLevel = node.attrs?.level;
      const level = typeof rawLevel === "number" ? Math.min(Math.max(rawLevel, 1), 6) : 1;
      return `${"#".repeat(level)} ${renderInline(node.content)}`.trim();
    }
    case "blockquote":
      return renderTiptapNodes(node.content ?? [])
        .split("\n")
        .map((line) => (line ? `> ${line}` : ">"))
        .join("\n");
    case "bulletList":
      return renderList(node, "-");
    case "orderedList":
      return renderList(node, "ordered");
    case "taskList":
      return renderList(node, "task");
    case "codeBlock":
      return renderCodeFence(renderInline(node.content), node.attrs);
    case "horizontalRule":
      return "---";
    case "image":
      return renderImageNode(node);
    case "hardBreak":
      return "\\\n";
    default:
      return renderInline(node.content);
  }
}

function renderList(node: TiptapNode, marker: string | "ordered" | "task"): string {
  const orderedStart =
    marker === "ordered" && typeof node.attrs?.start === "number" ? Number(node.attrs.start) : 1;
  return (node.content ?? [])
    .map((item, index) => {
      const prefix =
        marker === "ordered"
          ? `${orderedStart + index}. `
          : marker === "task"
            ? `- [${item.attrs?.checked ? "x" : " "}] `
            : `${marker} `;
      return prefixMultiline(renderInlineTextBlock(item), prefix);
    })
    .join("\n");
}

function renderInlineTextBlock(node: TiptapNode): string {
  const rendered = (node.content ?? [])
    .map((child) => ({ child, text: renderTiptapNode(child) }))
    .filter((entry) => Boolean(entry.text));
  if (rendered.length === 0) return renderInline(node.content);

  return rendered
    .map((entry, index) => {
      if (index === 0) return entry.text;
      return `${isListNode(entry.child) ? "\n" : "\n\n"}${entry.text}`;
    })
    .join("");
}

function isListNode(node: TiptapNode): boolean {
  return node.type === "bulletList" || node.type === "orderedList" || node.type === "taskList";
}

function renderInlineNode(node: TiptapNode): string {
  if (node.type === "text" && typeof node.text === "string") {
    return renderMarkedText(node.text, normalizeMarks(node.marks));
  }
  if (node.type === "image") return renderImageNode(node);
  if (node.type === "hardBreak") return "\\\n";
  return renderInline(node.content);
}

function parseMarkdownBlocks(lines: string[]): TiptapNode[] {
  const content: TiptapNode[] = [];
  let index = 0;
  let paragraph: string[] = [];

  function flushParagraph(): void {
    if (paragraph.length === 0) return;
    const inline = paragraph.flatMap((line, lineIndex) => {
      const nodes = parseInlineMarkdown(line);
      return lineIndex === paragraph.length - 1 ? nodes : [...nodes, { type: "hardBreak" }];
    });
    paragraph = [];
    if (inline.length === 0) return;
    pushParagraphOrImageBlocks(content, inline.length ? inline : [{ type: "text", text: "" }]);
  }

  while (index < lines.length) {
    const line = lines[index] ?? "";

    if (!line.trim()) {
      flushParagraph();
      index += 1;
      continue;
    }

    const fence = /^(`{3,})([^`]*)\s*$/.exec(line);
    if (fence) {
      flushParagraph();
      const codeLines: string[] = [];
      const fenceMarker = fence[1] ?? "```";
      const language = fence[2]?.trim() || null;
      index += 1;
      while (index < lines.length && (lines[index] ?? "").trim() !== fenceMarker) {
        codeLines.push(lines[index] ?? "");
        index += 1;
      }
      if (index < lines.length && (lines[index] ?? "").trim() === fenceMarker) index += 1;
      content.push({
        type: "codeBlock",
        attrs: language ? { language } : undefined,
        content: codeLines.length ? [{ type: "text", text: codeLines.join("\n") }] : undefined,
      });
      continue;
    }

    const heading = /^(#{1,6})\s+(.+?)\s*$/.exec(line);
    if (heading) {
      flushParagraph();
      content.push({
        type: "heading",
        attrs: { level: heading[1].length },
        content: parseInlineMarkdown(heading[2]),
      });
      index += 1;
      continue;
    }

    if (/^\s*(?:---+|\*\*\*+|___+)\s*$/.test(line)) {
      flushParagraph();
      content.push({ type: "horizontalRule" });
      index += 1;
      continue;
    }

    if (/^\s*>/.test(line)) {
      flushParagraph();
      const quoteLines: string[] = [];
      while (index < lines.length) {
        const quoteLine = lines[index] ?? "";
        if (!quoteLine.trim()) {
          quoteLines.push("");
          index += 1;
          continue;
        }
        const quoteMatch = /^\s*>\s?(.*)$/.exec(quoteLine);
        if (!quoteMatch) break;
        quoteLines.push(quoteMatch[1]);
        index += 1;
      }
      content.push({
        type: "blockquote",
        content: parseMarkdownBlocks(quoteLines),
      });
      continue;
    }

    const listBlock = parseListBlock(lines, index);
    if (listBlock) {
      flushParagraph();
      content.push(listBlock.node);
      index = listBlock.nextIndex;
      continue;
    }

    paragraph.push(line);
    index += 1;
  }

  flushParagraph();
  return content;
}

function pushParagraphOrImageBlocks(content: TiptapNode[], inline: TiptapNode[]): void {
  let paragraph: TiptapNode[] = [];

  function flush(): void {
    if (paragraph.length === 0) return;
    content.push({ type: "paragraph", content: paragraph });
    paragraph = [];
  }

  for (const node of inline) {
    if (node.type === "image" && !node.marks?.length) {
      flush();
      content.push(node);
      continue;
    }
    paragraph.push(node);
  }

  flush();
}

type ParsedListItem = {
  type: "taskList" | "bulletList" | "orderedList";
  text: string;
  checked?: boolean;
  start?: number;
  indent: number;
};

function parseListItem(line: string): ParsedListItem | null {
  const task = /^(\s*)[-*]\s+\[([ xX])\]\s*(.*)$/.exec(line);
  if (task) {
    return {
      type: "taskList",
      text: task[3],
      checked: task[2].toLowerCase() === "x",
      indent: task[1]?.length ?? 0,
    };
  }

  const bullet = /^(\s*)[-*]\s+(.+)$/.exec(line);
  if (bullet) {
    return {
      type: "bulletList",
      text: bullet[2],
      indent: bullet[1]?.length ?? 0,
    };
  }

  const ordered = /^(\s*)(\d+)[.)]\s+(.+)$/.exec(line);
  if (ordered) {
    return {
      type: "orderedList",
      text: ordered[3],
      start: Number(ordered[2]),
      indent: ordered[1]?.length ?? 0,
    };
  }

  return null;
}

function parseListBlock(
  lines: string[],
  startIndex: number,
): { node: TiptapNode; nextIndex: number } | null {
  const first = parseListItem(lines[startIndex] ?? "");
  if (!first) return null;

  const items: Array<{
    text: string;
    checked?: boolean;
    children: TiptapNode[];
  }> = [];
  let index = startIndex;

  while (index < lines.length) {
    const current = parseListItem(lines[index] ?? "");
    if (!current || current.type !== first.type || current.indent !== first.indent) break;

    const parts = [current.text.trim()];
    const children: TiptapNode[] = [];
    index += 1;
    while (index < lines.length) {
      const continuation = lines[index] ?? "";
      if (!continuation.trim()) break;
      const nestedItem = parseListItem(continuation);
      if (nestedItem) {
        if (nestedItem.indent > current.indent) {
          const nested = parseListBlock(lines, index);
          if (nested) {
            children.push(nested.node);
            index = nested.nextIndex;
            continue;
          }
        }
        break;
      }
      const continuationIndent = continuation.match(/^\s*/)?.[0].length ?? 0;
      if (continuationIndent <= current.indent) break;
      parts.push(continuation.trim());
      index += 1;
    }
    items.push({ text: parts.join(" "), checked: current.checked, children });
  }

  return {
    node: {
      type: first.type,
      attrs:
        first.type === "orderedList" && typeof first.start === "number" && first.start !== 1
          ? { start: first.start }
          : undefined,
      content: items.map((item) => ({
        type: first.type === "taskList" ? "taskItem" : "listItem",
        attrs: first.type === "taskList" ? { checked: item.checked === true } : undefined,
        content: [{ type: "paragraph", content: parseInlineMarkdown(item.text) }, ...item.children],
      })),
    },
    nextIndex: index,
  };
}

function parseInlineMarkdown(text: string): TiptapNode[] {
  const nodes: TiptapNode[] = [];
  let buffer = "";
  let index = 0;

  function flushBuffer(): void {
    if (!buffer) return;
    appendTextNode(nodes, buffer);
    buffer = "";
  }

  while (index < text.length) {
    if (text.startsWith("![", index)) {
      const image = parseMarkdownLinkLike(text.slice(index), true);
      if (image) {
        flushBuffer();
        nodes.push({
          type: "image",
          attrs: {
            src: image.href,
            alt: image.label,
            title: image.title ?? undefined,
          },
        });
        index += image.length;
        continue;
      }
    }

    if (text[index] === "[") {
      const link = parseMarkdownLinkLike(text.slice(index), false);
      if (link) {
        flushBuffer();
        nodes.push(
          ...applyMark(parseInlineMarkdown(link.label), {
            type: "link",
            attrs: { href: link.href, title: link.title ?? undefined },
          }),
        );
        index += link.length;
        continue;
      }
    }

    if (text[index] === "`") {
      const closing = text.indexOf("`", index + 1);
      if (closing > index + 1) {
        flushBuffer();
        appendTextNode(nodes, text.slice(index + 1, closing), [{ type: "code" }]);
        index = closing + 1;
        continue;
      }
    }

    const strong = matchDelimited(text, index, ["**", "__"]);
    if (strong) {
      flushBuffer();
      nodes.push(...applyMark(parseInlineMarkdown(strong.inner), { type: "bold" }));
      index = strong.nextIndex;
      continue;
    }

    const strike = matchDelimited(text, index, ["~~"]);
    if (strike) {
      flushBuffer();
      nodes.push(...applyMark(parseInlineMarkdown(strike.inner), { type: "strike" }));
      index = strike.nextIndex;
      continue;
    }

    const emphasis = matchDelimited(text, index, ["*", "_"], true);
    if (emphasis) {
      flushBuffer();
      nodes.push(...applyMark(parseInlineMarkdown(emphasis.inner), { type: "italic" }));
      index = emphasis.nextIndex;
      continue;
    }

    buffer += text[index];
    index += 1;
  }

  flushBuffer();
  return nodes;
}

function matchDelimited(
  text: string,
  index: number,
  delimiters: string[],
  single = false,
): { inner: string; nextIndex: number } | null {
  for (const delimiter of delimiters) {
    if (!text.startsWith(delimiter, index)) continue;
    if (single && text.startsWith(delimiter.repeat(2), index)) continue;
    const closing = text.indexOf(delimiter, index + delimiter.length);
    if (closing <= index + delimiter.length) continue;
    return {
      inner: text.slice(index + delimiter.length, closing),
      nextIndex: closing + delimiter.length,
    };
  }
  return null;
}

function parseMarkdownLinkLike(
  text: string,
  image: boolean,
): {
  label: string;
  href: string;
  title: string | null;
  length: number;
} | null {
  const pattern = image
    ? /^!\[([^\]]*)\]\((\S+?)(?:\s+"([^"]*)")?\)/
    : /^\[([^\]]+)\]\((\S+?)(?:\s+"([^"]*)")?\)/;
  const match = pattern.exec(text);
  if (!match) return null;
  return {
    label: match[1] ?? "",
    href: match[2] ?? "",
    title: match[3] ?? null,
    length: match[0].length,
  };
}

function appendTextNode(nodes: TiptapNode[], text: string, marks?: TiptapMark[]): void {
  if (!text) return;
  const normalizedMarks = normalizeMarks(marks);
  const last = nodes.at(-1);
  if (
    last?.type === "text" &&
    typeof last.text === "string" &&
    JSON.stringify(normalizeMarks(last.marks)) === JSON.stringify(normalizedMarks)
  ) {
    last.text += text;
    return;
  }
  nodes.push({
    type: "text",
    text,
    marks: normalizedMarks.length > 0 ? normalizedMarks : undefined,
  });
}

function applyMark(nodes: TiptapNode[], mark: TiptapMark): TiptapNode[] {
  return nodes.map((node) => {
    if (node.type === "text") {
      return {
        ...node,
        marks: mergeMarks(normalizeMarks(node.marks), mark),
      };
    }
    return node.content ? { ...node, content: applyMark(node.content, mark) } : node;
  });
}

function normalizeMarks(marks: unknown): TiptapMark[] {
  if (!Array.isArray(marks)) return [];
  return marks
    .filter((mark): mark is TiptapMark => !!mark && typeof mark === "object")
    .map((mark) => ({
      type: typeof mark.type === "string" ? mark.type : undefined,
      attrs:
        mark.attrs && typeof mark.attrs === "object"
          ? (mark.attrs as Record<string, unknown>)
          : undefined,
    }))
    .filter((mark) => typeof mark.type === "string");
}

function mergeMarks(existing: TiptapMark[], next: TiptapMark): TiptapMark[] {
  return [...existing.filter((mark) => mark.type !== next.type), next];
}

function renderMarkedText(text: string, marks: TiptapMark[]): string {
  const link = marks.find((mark) => mark.type === "link");
  const code = marks.find((mark) => mark.type === "code");
  const emphasis = marks.some((mark) => mark.type === "italic" || mark.type === "em");
  const strong = marks.some((mark) => mark.type === "bold" || mark.type === "strong");
  const strike = marks.some((mark) => mark.type === "strike");

  let rendered = code ? wrapInlineCode(text) : escapeMarkdownText(text);
  if (!code && strong) rendered = `**${rendered}**`;
  if (!code && emphasis) rendered = `*${rendered}*`;
  if (!code && strike) rendered = `~~${rendered}~~`;
  if (link?.attrs && typeof link.attrs.href === "string") {
    rendered = `[${rendered}](${link.attrs.href})`;
  }
  return rendered;
}

function wrapInlineCode(text: string): string {
  if (!text.includes("`")) return `\`${text}\``;
  return `\`\` ${text} \`\``;
}

function escapeMarkdownText(text: string): string {
  return text.replace(/([\\\[\]\*_~])/g, "\\$1");
}

function renderCodeFence(text: string, attrs: unknown): string {
  const rawLanguage =
    attrs && typeof attrs === "object" ? (attrs as Record<string, unknown>).language : undefined;
  const language = typeof rawLanguage === "string" ? rawLanguage.trim() : "";
  const maxBackticks = Math.max(0, ...Array.from(text.matchAll(/`+/g), (match) => match[0].length));
  const fence = "`".repeat(Math.max(3, maxBackticks + 1));
  return `${fence}${language}\n${text}\n${fence}`;
}

function renderImageNode(node: TiptapNode): string {
  const src = typeof node.attrs?.src === "string" ? node.attrs.src : "";
  const alt = typeof node.attrs?.alt === "string" ? node.attrs.alt : "";
  const title = typeof node.attrs?.title === "string" ? ` "${node.attrs.title}"` : "";
  return src ? `![${alt}](${src}${title})` : "";
}
