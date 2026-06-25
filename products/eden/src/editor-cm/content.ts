const MARKDOWN_CONTENT_VERSION = 1;

export interface MarkdownContent {
  type: "markdown";
  version: typeof MARKDOWN_CONTENT_VERSION;
  text: string;
}

export function isMarkdownContent(value: unknown): value is MarkdownContent {
  return !!value && typeof value === "object" && (value as { type?: unknown }).type === "markdown";
}

export function writeEntryMarkdown(md: string): MarkdownContent {
  return {
    type: "markdown",
    version: MARKDOWN_CONTENT_VERSION,
    text: md,
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

  return legacyProseMirrorToText(value);
}

export function legacyProseMirrorToText(value: unknown): string {
  try {
    return extractLegacyNode(value).trim();
  } catch {
    return "";
  }
}

type LegacyNode = {
  type?: unknown;
  text?: unknown;
  attrs?: unknown;
  content?: unknown;
};

function extractLegacyNode(value: unknown): string {
  if (!value || typeof value !== "object") return "";

  const node = value as LegacyNode;
  if (typeof node.text === "string") return node.text;

  const media = mediaMarkdown(node);
  if (media) return media;

  const children = Array.isArray(node.content)
    ? node.content.map(extractLegacyNode).filter(Boolean)
    : [];
  const type = typeof node.type === "string" ? node.type : "";

  switch (type) {
    case "doc":
      return joinBlocks(children);
    case "paragraph":
      return joinInline(children);
    case "heading":
      return joinInline(children);
    case "blockquote":
      return joinBlocks(children)
        .split("\n")
        .map((line) => (line ? `> ${line}` : ">"))
        .join("\n");
    case "bulletList":
    case "orderedList":
    case "taskList":
      return children.join("\n");
    case "listItem":
      return children
        .join("\n")
        .split("\n")
        .map((line, index) => (index === 0 ? `- ${line}` : `  ${line}`))
        .join("\n");
    case "codeBlock":
      return `\`\`\`\n${joinInline(children)}\n\`\`\``;
    case "hardBreak":
    case "hard_break":
      return "\n";
    default:
      return children.length > 0 ? joinBlocks(children) : "";
  }
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
