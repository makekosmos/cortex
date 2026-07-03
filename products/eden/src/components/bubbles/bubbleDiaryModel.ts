import type { JSONContent } from "@tiptap/core";
import { readEntryTiptapDoc, type TiptapDoc, type TiptapNode } from "../../editor-content/content";
import { SYSTEM_TYPE_JOURNAL_ID, SYSTEM_TYPE_NOTE_ID } from "../../lib/systemTypeDefinitions";

export type BubbleKind = "plain" | "idea" | "task" | "highlight";

export interface BubbleTimelineNode {
  id: string;
  date?: string;
  time: string;
  text: string;
  contentJson?: JSONContent;
  tags: string[];
  kind: BubbleKind;
}

export const LOCAL_BUBBLES_STORAGE_KEY = "eden-bubble-diary-local-bubbles";
export const LOCAL_BUBBLES_STORAGE_VERSION = 1;

export const BUBBLE_KIND_OPTIONS: { value: BubbleKind; label: string; color: string }[] = [
  { value: "plain", label: "Просто", color: "var(--border-color-strong, var(--border))" },
  { value: "idea", label: "Идея", color: "#017AFF" },
  { value: "task", label: "Задача", color: "#4de64d" },
  { value: "highlight", label: "Подсветить", color: "#FF703A" },
];

const TAG_PATTERN = /#[\p{L}\p{N}_-]+/gu;
const DATE_KEY_PATTERN = /^\d{4}-\d{2}-\d{2}$/;

const BLOCK_NODE_TYPES = new Set(["paragraph", "heading", "blockquote", "codeBlock", "listItem"]);

type JournalEntryLike = Pick<
  Entry,
  "id" | "title" | "type_id" | "content_json" | "created_at" | "updated_at" | "deleted_at"
>;

export function parseBubbleDraft(input: string): {
  text: string;
  tags: string[];
} {
  const tags: string[] = [];
  const text = input
    .replace(TAG_PATTERN, (match) => {
      const tag = match.slice(1).trim().toLowerCase();
      if (tag) tags.push(tag);
      return " ";
    })
    .replace(/\s+/g, " ")
    .trim();

  return {
    text,
    tags: [...new Set(tags)],
  };
}

export function plainTextToTiptapDoc(text: string): JSONContent {
  return {
    type: "doc",
    content: [
      {
        type: "paragraph",
        content: text ? [{ type: "text", text }] : [],
      },
    ],
  };
}

export function createDraftBubble(
  input: string | JSONContent,
  now = new Date(),
  plainText = typeof input === "string" ? input : tiptapPlainText(input),
): BubbleTimelineNode | null {
  const contentJson = typeof input === "string" ? plainTextToTiptapDoc(input) : input;
  const draft = parseBubbleDraft(plainText);
  if (!draft.text) return null;

  return {
    id: `draft-${now.getTime()}`,
    date: formatBubbleDateKey(now),
    time: `${String(now.getHours()).padStart(2, "0")}:${String(now.getMinutes()).padStart(2, "0")}`,
    text: draft.text,
    contentJson: stripTagsFromTiptapDoc(contentJson),
    tags: draft.tags,
    kind: "plain",
  };
}

export function createJournalBubblesFromEntry(entry: JournalEntryLike): BubbleTimelineNode[] {
  if (!isLegacyDatedJournalEntry(entry)) return [];

  const date = journalDate(entry);
  const doc = readEntryTiptapDoc(entry.content_json);
  return (doc.content ?? []).flatMap((block, index) => {
    const contentJson = docFromBlock(block);
    const text = tiptapPlainText(contentJson);
    const bubble = createBubbleFromContent(contentJson, {
      id: `journal-${entry.id}-${index}`,
      date,
      time: date,
      plainText: text,
    });
    return bubble ? [bubble] : [];
  });
}

export function isLegacyDatedJournalEntry(entry: JournalEntryLike): boolean {
  return (
    (entry.type_id === SYSTEM_TYPE_JOURNAL_ID ||
      entry.type_id === SYSTEM_TYPE_NOTE_ID ||
      entry.type_id === null) &&
    entry.deleted_at === null &&
    DATE_KEY_PATTERN.test(entry.title.trim())
  );
}

export function encodeLocalBubblesStorage(
  bubbles: BubbleTimelineNode[],
  options: { journalImported?: boolean } = {},
): string {
  return JSON.stringify({
    version: LOCAL_BUBBLES_STORAGE_VERSION,
    journalImported: options.journalImported === true,
    bubbles,
  });
}

export function decodeLocalBubblesStorage(value: unknown): BubbleTimelineNode[] {
  if (!value || typeof value !== "object") return [];
  const record = value as Record<string, unknown>;
  if (record.version !== LOCAL_BUBBLES_STORAGE_VERSION) return [];
  return normalizeLocalBubbles(record.bubbles);
}

export function normalizeBubbleKind(value: unknown): BubbleKind {
  return BUBBLE_KIND_OPTIONS.some((option) => option.value === value)
    ? (value as BubbleKind)
    : "plain";
}

export function normalizeLocalBubbles(value: unknown): BubbleTimelineNode[] {
  if (!Array.isArray(value)) return [];

  return value.flatMap((node) => {
    if (!node || typeof node !== "object") return [];
    const record = node as Record<string, unknown>;
    if (
      typeof record.id !== "string" ||
      typeof record.time !== "string" ||
      typeof record.text !== "string" ||
      !Array.isArray(record.tags)
    ) {
      return [];
    }

    return [
      {
        id: record.id,
        date: normalizeBubbleDateKey(record.date, record.time),
        time: record.time,
        text: record.text,
        contentJson: normalizeTiptapDoc(record.contentJson, record.text),
        tags: record.tags.filter((tag): tag is string => typeof tag === "string"),
        kind: normalizeBubbleKind(record.kind),
      },
    ];
  });
}

export function bubbleDateKey(node: BubbleTimelineNode, fallback = new Date()): string {
  return normalizeBubbleDateKey(node.date, node.time) ?? formatBubbleDateKey(fallback);
}

export function formatBubbleDateKey(date: Date): string {
  return [
    String(date.getFullYear()).padStart(4, "0"),
    String(date.getMonth() + 1).padStart(2, "0"),
    String(date.getDate()).padStart(2, "0"),
  ].join("-");
}

function createBubbleFromContent(
  contentJson: JSONContent,
  opts: { id: string; date?: string; time: string; plainText: string },
): BubbleTimelineNode | null {
  const draft = parseBubbleDraft(opts.plainText);
  if (!draft.text) return null;

  return {
    id: opts.id,
    date: normalizeBubbleDateKey(opts.date, opts.time),
    time: opts.time,
    text: draft.text,
    contentJson: stripTagsFromTiptapDoc(contentJson),
    tags: draft.tags,
    kind: "plain",
  };
}

function docFromBlock(block: TiptapNode): TiptapDoc {
  return { type: "doc", content: [block] };
}

function journalDate(entry: JournalEntryLike): string {
  const titleDate = entry.title.trim().match(/^\d{4}-\d{2}-\d{2}/)?.[0];
  if (titleDate) return titleDate;

  const timestamp = entry.created_at || entry.updated_at;
  const date = new Date(timestamp);
  if (Number.isNaN(date.getTime())) return "";
  return date.toISOString().slice(0, 10);
}

function normalizeTiptapDoc(value: unknown, fallbackText: string): JSONContent {
  if (!value || typeof value !== "object") return plainTextToTiptapDoc(fallbackText);
  const record = value as JSONContent;
  return record.type === "doc" ? record : plainTextToTiptapDoc(fallbackText);
}

function tiptapPlainText(node: JSONContent): string {
  const chunks: string[] = [];
  collectTiptapText(node, chunks);
  return chunks
    .join("")
    .replace(/\n{3,}/g, "\n\n")
    .trim();
}

function collectTiptapText(node: JSONContent, chunks: string[]): void {
  if (node.type === "text") {
    chunks.push(node.text ?? "");
    return;
  }

  for (const child of node.content ?? []) {
    collectTiptapText(child, chunks);
  }

  if (node.type && BLOCK_NODE_TYPES.has(node.type)) {
    chunks.push("\n");
  }
}

function normalizeBubbleDateKey(date: unknown, time: unknown): string | undefined {
  if (typeof date === "string" && DATE_KEY_PATTERN.test(date)) return date;
  if (typeof time === "string" && DATE_KEY_PATTERN.test(time)) return time;
  return undefined;
}

function stripTagsFromTiptapDoc(doc: JSONContent): JSONContent {
  return stripTagsFromTiptapNode(doc, false) ?? plainTextToTiptapDoc("");
}

function stripTagsFromTiptapNode(node: JSONContent, insideCodeBlock: boolean): JSONContent | null {
  const nextInsideCodeBlock = insideCodeBlock || node.type === "codeBlock";
  if (node.type === "text") {
    const text = nextInsideCodeBlock ? node.text : node.text?.replace(TAG_PATTERN, " ");
    if (!text?.trim()) return null;
    return { ...node, text };
  }

  const content = (node.content ?? [])
    .map((child) => stripTagsFromTiptapNode(child, nextInsideCodeBlock))
    .filter((child): child is JSONContent => Boolean(child));

  return { ...node, content: content.length > 0 ? content : undefined };
}
