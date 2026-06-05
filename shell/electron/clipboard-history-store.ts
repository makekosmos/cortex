import type { ClipboardHistoryItem } from "../shared/ipc-types";

const DEFAULT_MAX_ITEMS = 60;
const PREVIEW_LIMIT = 180;

export interface ClipboardHistoryStore {
  record(text: string): ClipboardHistoryItem | null;
  recordImage(args: {
    dataUrl: string;
    width: number;
    height: number;
    mimeType?: string;
  }): ClipboardHistoryItem | null;
  recordFile(path: string): ClipboardHistoryItem | null;
  updateTimestamp(id: string): ClipboardHistoryItem | null;
  togglePin(id: string): ClipboardHistoryItem | null;
  clearAll(): void;
  clearUnpinned(): void;
  clear(): void;
  remove(id: string): boolean;
  find(id: string): ClipboardHistoryItem | null;
  list(): ClipboardHistoryItem[];
}

type ClipboardTextKind = "text" | "link" | "color";

interface TextClassification {
  kind: ClipboardTextKind;
  text: string;
  preview: string;
  searchText: string;
  url?: string;
  color?: string;
}

export function classifyClipboardText(text: string): TextClassification | null {
  const normalized = normalizeClipboardText(text);
  if (!normalized) return null;

  const url = normalized.match(/^https?:\/\/[^\s]+$/i)?.[0];
  if (url) {
    return {
      kind: "link",
      text: normalized,
      preview: normalized,
      searchText: normalized,
      url,
    };
  }

  const color = normalizeColor(normalized);
  if (color) {
    return {
      kind: "color",
      text: color,
      preview: color,
      searchText: `${color} ${normalized}`,
      color,
    };
  }

  return {
    kind: "text",
    text: normalized,
    preview: previewText(normalized),
    searchText: normalized,
  };
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
    const classified = classifyClipboardText(text);
    if (!classified) return null;

    const existing = items.find(
      (item) =>
        (item.kind === "text" || item.kind === "link" || item.kind === "color") &&
        item.text === classified.text,
    );
    if (existing) return touch(existing);

    const timestamp = now();
    const item: ClipboardHistoryItem = {
      id: `clip-${nextId++}`,
      kind: classified.kind,
      text: classified.text,
      preview: classified.preview,
      createdAt: timestamp,
      updatedAt: timestamp,
      charCount: classified.text.length,
      pinned: false,
      searchText: classified.searchText,
      url: classified.url,
      color: classified.color,
    };
    insert(item);
    return item;
  }

  function recordImage(args: {
    dataUrl: string;
    width: number;
    height: number;
    mimeType?: string;
  }): ClipboardHistoryItem | null {
    if (!args.dataUrl || args.width <= 0 || args.height <= 0) return null;

    const existing = items.find(
      (item) => item.kind === "image" && item.imageDataUrl === args.dataUrl,
    );
    if (existing) return touch(existing);

    const timestamp = now();
    const item: ClipboardHistoryItem = {
      id: `clip-${nextId++}`,
      kind: "image",
      text: "",
      preview: `Изображение ${args.width}×${args.height}`,
      createdAt: timestamp,
      updatedAt: timestamp,
      charCount: 0,
      pinned: false,
      searchText: `изображение image ${args.width} ${args.height}`,
      imageDataUrl: args.dataUrl,
      width: args.width,
      height: args.height,
      mimeType: args.mimeType,
    };
    insert(item);
    return item;
  }

  function recordFile(filePath: string): ClipboardHistoryItem | null {
    const normalized = filePath.trim();
    if (!normalized) return null;
    const existing = items.find((item) => item.kind === "file" && item.filePath === normalized);
    if (existing) return touch(existing);

    const fileName = normalized.split(/[\\/]/).filter(Boolean).at(-1) ?? normalized;
    const timestamp = now();
    const item: ClipboardHistoryItem = {
      id: `clip-${nextId++}`,
      kind: "file",
      text: normalized,
      preview: fileName,
      createdAt: timestamp,
      updatedAt: timestamp,
      charCount: normalized.length,
      pinned: false,
      searchText: `${fileName} ${normalized}`,
      filePath: normalized,
      fileName,
    };
    insert(item);
    return item;
  }

  function touch(existing: ClipboardHistoryItem): ClipboardHistoryItem {
    const updated = { ...existing, updatedAt: now() };
    items = [updated, ...items.filter((item) => item.id !== existing.id)];
    sortItems();
    return updated;
  }

  function insert(item: ClipboardHistoryItem): void {
    items = [item, ...items];
    prune();
    sortItems();
  }

  function sortItems(): void {
    items = [...items].sort(
      (a, b) => Number(b.pinned) - Number(a.pinned) || b.updatedAt - a.updatedAt,
    );
  }

  function prune(): void {
    const pinned = items.filter((item) => item.pinned);
    const unpinned = items
      .filter((item) => !item.pinned)
      .slice(0, Math.max(0, maxItems - pinned.length));
    items = [...pinned, ...unpinned];
  }

  return {
    record,
    recordImage,
    recordFile,
    list: () => [...items],
    find: (id) => items.find((item) => item.id === id) ?? null,
    updateTimestamp: (id) => {
      const item = items.find((candidate) => candidate.id === id);
      return item ? touch(item) : null;
    },
    togglePin: (id) => {
      const item = items.find((candidate) => candidate.id === id);
      if (!item) return null;
      const updated = { ...item, pinned: !item.pinned, updatedAt: now() };
      items = [updated, ...items.filter((candidate) => candidate.id !== id)];
      sortItems();
      return updated;
    },
    remove: (id) => {
      const before = items.length;
      items = items.filter((item) => item.id !== id);
      return items.length !== before;
    },
    clearUnpinned: () => {
      items = items.filter((item) => item.pinned);
    },
    clear: () => {
      items = items.filter((item) => item.pinned);
    },
    clearAll: () => {
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

function normalizeColor(value: string): string | null {
  const trimmed = value.trim();
  const hex = trimmed.match(/^#?([0-9a-f]{3}|[0-9a-f]{6}|[0-9a-f]{8})$/i)?.[1];
  if (hex) return `#${hex.toUpperCase()}`;

  const rgb = trimmed.match(
    /^rgba?\(\s*(\d{1,3})\s*,\s*(\d{1,3})\s*,\s*(\d{1,3})(?:\s*,\s*(0|1|0?\.\d+))?\s*\)$/i,
  );
  if (!rgb) return null;
  const parts = rgb.slice(1, 4).map((part) => Number(part));
  if (parts.some((part) => !Number.isInteger(part) || part < 0 || part > 255)) return null;
  return `#${parts
    .map((part) => part.toString(16).padStart(2, "0"))
    .join("")
    .toUpperCase()}`;
}
