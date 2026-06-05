import type { ClipboardHistoryItem } from "../shared/ipc-types";

const DEFAULT_MAX_ITEMS = 60;
const PREVIEW_LIMIT = 180;

export interface ClipboardHistoryStore {
  record(text: string): ClipboardHistoryItem | null;
  recordImage(args: {
    dataUrl: string;
    width: number;
    height: number;
  }): ClipboardHistoryItem | null;
  list(): ClipboardHistoryItem[];
  find(id: string): ClipboardHistoryItem | null;
  remove(id: string): boolean;
  clear(): void;
}

export function createClipboardHistoryStore(options?: {
  maxItems?: number;
  now?: () => number;
}): ClipboardHistoryStore {
  const maxItems = options?.maxItems ?? DEFAULT_MAX_ITEMS;
  const now = options?.now ?? Date.now;
  let items: ClipboardHistoryItem[] = [];
  let nextId = 1;

  function record(text: string): ClipboardHistoryItem | null {
    const normalized = normalizeClipboardText(text);
    if (!normalized) return null;

    const existing = items.find((item) => item.kind === "text" && item.text === normalized);
    if (existing) {
      const updated = { ...existing, createdAt: now() };
      items = [updated, ...items.filter((item) => item.id !== existing.id)];
      return updated;
    }

    const item: ClipboardHistoryItem = {
      id: `clip-${nextId++}`,
      kind: "text",
      text: normalized,
      preview: previewText(normalized),
      createdAt: now(),
      charCount: normalized.length,
    };
    items = [item, ...items].slice(0, maxItems);
    return item;
  }

  function recordImage(args: {
    dataUrl: string;
    width: number;
    height: number;
  }): ClipboardHistoryItem | null {
    if (!args.dataUrl || args.width <= 0 || args.height <= 0) return null;

    const existing = items.find(
      (item) => item.kind === "image" && item.imageDataUrl === args.dataUrl,
    );
    if (existing) {
      const updated = { ...existing, createdAt: now() };
      items = [updated, ...items.filter((item) => item.id !== existing.id)];
      return updated;
    }

    const item: ClipboardHistoryItem = {
      id: `clip-${nextId++}`,
      kind: "image",
      text: "",
      preview: `Изображение ${args.width}×${args.height}`,
      createdAt: now(),
      charCount: 0,
      imageDataUrl: args.dataUrl,
      width: args.width,
      height: args.height,
    };
    items = [item, ...items].slice(0, maxItems);
    return item;
  }

  return {
    record,
    recordImage,
    list: () => [...items],
    find: (id) => items.find((item) => item.id === id) ?? null,
    remove: (id) => {
      const before = items.length;
      items = items.filter((item) => item.id !== id);
      return items.length !== before;
    },
    clear: () => {
      items = [];
    },
  };
}

function normalizeClipboardText(text: string): string {
  return text.replace(/\r\n/g, "\n").trim();
}

function previewText(text: string): string {
  const compact = text.replace(/\s+/g, " ");
  if (compact.length <= PREVIEW_LIMIT) return compact;
  return `${compact.slice(0, PREVIEW_LIMIT - 1)}…`;
}
