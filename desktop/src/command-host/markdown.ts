export type CommandMarkdownBlock =
  | { type: "heading"; level: 1 | 2 | 3; text: string }
  | { type: "paragraph"; text: string }
  | { type: "list"; items: string[] }
  | { type: "code"; text: string };

function flushParagraph(lines: string[], blocks: CommandMarkdownBlock[]): void {
  if (lines.length === 0) return;
  blocks.push({ type: "paragraph", text: lines.join(" ") });
  lines.length = 0;
}

function flushList(items: string[], blocks: CommandMarkdownBlock[]): void {
  if (items.length === 0) return;
  blocks.push({ type: "list", items: [...items] });
  items.length = 0;
}

export function parseCommandMarkdown(input: string): CommandMarkdownBlock[] {
  const normalized = input.replace(/\r\n?/g, "\n");
  const lines = normalized.split("\n");
  const blocks: CommandMarkdownBlock[] = [];
  const paragraph: string[] = [];
  const listItems: string[] = [];
  let codeLines: string[] | null = null;

  for (const rawLine of lines) {
    const line = rawLine.trimEnd();
    const trimmed = line.trim();

    if (trimmed.startsWith("```")) {
      if (codeLines) {
        blocks.push({ type: "code", text: codeLines.join("\n") });
        codeLines = null;
      } else {
        flushParagraph(paragraph, blocks);
        flushList(listItems, blocks);
        codeLines = [];
      }
      continue;
    }

    if (codeLines) {
      codeLines.push(rawLine);
      continue;
    }

    if (!trimmed) {
      flushParagraph(paragraph, blocks);
      flushList(listItems, blocks);
      continue;
    }

    const heading = /^(#{1,3})\s+(.+)$/.exec(trimmed);
    if (heading) {
      flushParagraph(paragraph, blocks);
      flushList(listItems, blocks);
      blocks.push({
        type: "heading",
// SAFETY: the surrounding domain validation preserves the asserted contract.
        level: heading[1].length as 1 | 2 | 3,
        text: heading[2].trim(),
      });
      continue;
    }

    const listItem = /^[-*]\s+(.+)$/.exec(trimmed);
    if (listItem) {
      flushParagraph(paragraph, blocks);
      listItems.push(listItem[1].trim());
      continue;
    }

    flushList(listItems, blocks);
    paragraph.push(trimmed);
  }

  if (codeLines) blocks.push({ type: "code", text: codeLines.join("\n") });
  flushParagraph(paragraph, blocks);
  flushList(listItems, blocks);
  return blocks;
}
