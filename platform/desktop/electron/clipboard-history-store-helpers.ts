import type {
  ClipboardHistoryItem,
  ClipboardHistorySettings,
  ClipboardHistorySettingsPatch,
  ClipboardHistoryStats,
} from "../shared/ipc-types";

const DEFAULT_RETENTION_DAYS = 30;
const DEFAULT_MAX_BYTES = 64 * 1024 * 1024;
const PREVIEW_LIMIT = 180;
const DAY_MS = 24 * 60 * 60 * 1000;

export interface ClipboardHistoryRecordMetadata {
  source?: string;
  sourceIcon?: string;
}

type ClipboardTextKind = "text" | "link" | "color";

export interface TextClassification {
  kind: ClipboardTextKind;
  text: string;
  preview: string;
  searchText: string;
  url?: string;
  color?: string;
}

export interface PersistedClipboardHistory {
  version: 1;
  nextId: number;
  settings: ClipboardHistorySettings;
  items: ClipboardHistoryItem[];
}

export interface ClipboardHistoryStore {
  record(text: string, metadata?: ClipboardHistoryRecordMetadata): ClipboardHistoryItem | null;
  recordImage(args: {
    dataUrl: string;
    width: number;
    height: number;
    mimeType?: string;
    source?: string;
    sourceIcon?: string;
  }): ClipboardHistoryItem | null;
  recordFile(path: string, metadata?: ClipboardHistoryRecordMetadata): ClipboardHistoryItem | null;
  updateTimestamp(id: string): ClipboardHistoryItem | null;
  togglePin(id: string): ClipboardHistoryItem | null;
  clearAll(): void;
  clearUnpinned(): void;
  clear(): void;
  remove(id: string): boolean;
  find(id: string): ClipboardHistoryItem | null;
  list(): ClipboardHistoryItem[];
  settings(): ClipboardHistorySettings;
  updateSettings(patch: ClipboardHistorySettingsPatch): ClipboardHistorySettings;
  flush(): Promise<void>;
  stats(): ClipboardHistoryStats;
  pruneNow(): void;
}

export function defaultClipboardHistorySettings(): ClipboardHistorySettings {
  return {
    retentionDays: DEFAULT_RETENTION_DAYS,
    maxBytes: DEFAULT_MAX_BYTES,
  };
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

function normalizeClipboardText(text: string): string {
  return text.replace(/\r\n/g, "\n").trim();
}

export function normalizeSource(source: string | undefined): string | undefined {
  const value = source?.trim();
  return value ? value : undefined;
}

export function normalizeSourceIcon(sourceIcon: string | undefined): string | undefined {
  const value = sourceIcon?.trim();
  return value?.startsWith("data:image/") ? value : undefined;
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

export function normalizeSettings(input: unknown): ClipboardHistorySettings {
  const defaults = defaultClipboardHistorySettings();
  if (!input || typeof input !== "object" || Array.isArray(input)) return defaults;
  const raw = input as Partial<ClipboardHistorySettings>;
  return {
    retentionDays:
      typeof raw.retentionDays === "number" && Number.isFinite(raw.retentionDays)
        ? Math.max(0, Math.floor(raw.retentionDays))
        : defaults.retentionDays,
    maxBytes:
      typeof raw.maxBytes === "number" && Number.isFinite(raw.maxBytes)
        ? Math.max(0, Math.floor(raw.maxBytes))
        : defaults.maxBytes,
  };
}

export function normalizeNextId(input: unknown, items: ClipboardHistoryItem[]): number {
  if (typeof input === "number" && Number.isFinite(input) && input > 0) return Math.floor(input);
  const maxId = items.reduce((max, item) => {
    const parsed = item.id.match(/^clip-(\d+)$/)?.[1];
    return parsed ? Math.max(max, Number(parsed)) : max;
  }, 0);
  return maxId + 1;
}

export function normalizeItem(input: unknown): ClipboardHistoryItem {
  const raw = input as Partial<ClipboardHistoryItem>;
  return withStorageBytes({
    id: typeof raw.id === "string" && raw.id ? raw.id : `clip-${Date.now()}`,
    kind: isKind(raw.kind) ? raw.kind : "text",
    text: typeof raw.text === "string" ? raw.text : "",
    preview: typeof raw.preview === "string" ? raw.preview : "",
    createdAt: typeof raw.createdAt === "number" ? raw.createdAt : Date.now(),
    updatedAt: typeof raw.updatedAt === "number" ? raw.updatedAt : Date.now(),
    charCount: typeof raw.charCount === "number" ? raw.charCount : (raw.text ?? "").length,
    pinned: raw.pinned === true,
    searchText: typeof raw.searchText === "string" ? raw.searchText : (raw.text ?? ""),
    source: typeof raw.source === "string" ? raw.source : undefined,
    sourceIcon: typeof raw.sourceIcon === "string" ? raw.sourceIcon : undefined,
    imageDataUrl: typeof raw.imageDataUrl === "string" ? raw.imageDataUrl : undefined,
    width: typeof raw.width === "number" ? raw.width : undefined,
    height: typeof raw.height === "number" ? raw.height : undefined,
    url: typeof raw.url === "string" ? raw.url : undefined,
    color: typeof raw.color === "string" ? raw.color : undefined,
    filePath: typeof raw.filePath === "string" ? raw.filePath : undefined,
    fileName: typeof raw.fileName === "string" ? raw.fileName : undefined,
    mimeType: typeof raw.mimeType === "string" ? raw.mimeType : undefined,
  });
}

function isKind(value: unknown): value is ClipboardHistoryItem["kind"] {
  return (
    value === "text" ||
    value === "image" ||
    value === "link" ||
    value === "color" ||
    value === "file"
  );
}

export function isItem(item: ClipboardHistoryItem): boolean {
  return Boolean(item.id && item.preview !== undefined && item.searchText !== undefined);
}

export function withStorageBytes(item: ClipboardHistoryItem): ClipboardHistoryItem {
  const { storageBytes: _storageBytes, ...rest } = item;
  return {
    ...rest,
    storageBytes: storageBytesForItem(rest),
  };
}

function storageBytesForItem(item: Omit<ClipboardHistoryItem, "storageBytes">): number {
  return Buffer.byteLength(JSON.stringify(item), "utf8");
}

export function totalStorageBytes(items: ClipboardHistoryItem[]): number {
  return items.reduce((sum, item) => sum + (item.storageBytes ?? storageBytesForItem(item)), 0);
}

function sortClipboardHistoryItems(source: ClipboardHistoryItem[]): ClipboardHistoryItem[] {
  return [...source].sort(
    (a, b) => Number(b.pinned) - Number(a.pinned) || b.updatedAt - a.updatedAt,
  );
}

export function pruneClipboardHistoryItems(
  source: ClipboardHistoryItem[],
  settings: ClipboardHistorySettings,
  maxItems: number,
  pruneNow: number,
): ClipboardHistoryItem[] {
  const retentionDays = Math.floor(settings.retentionDays);
  const cutoff = retentionDays > 0 ? pruneNow - retentionDays * DAY_MS : Number.NEGATIVE_INFINITY;
  let next = source.filter((item) => item.pinned || item.updatedAt >= cutoff);

  const pinned = next.filter((item) => item.pinned);
  const unpinned = next
    .filter((item) => !item.pinned)
    .slice(0, Math.max(0, maxItems - pinned.length));
  next = sortClipboardHistoryItems([...pinned, ...unpinned]);

  if (settings.maxBytes > 0) {
    next = pruneClipboardHistoryItemsBySize(next, settings.maxBytes);
  }
  return next;
}

function pruneClipboardHistoryItemsBySize(
  source: ClipboardHistoryItem[],
  maxBytes: number,
): ClipboardHistoryItem[] {
  let total = totalStorageBytes(source);
  if (total <= maxBytes) return source;

  const removable = source.filter((item) => !item.pinned).sort((a, b) => a.updatedAt - b.updatedAt);
  const removed = new Set<string>();
  for (const item of removable) {
    if (total <= maxBytes) break;
    removed.add(item.id);
    total -= item.storageBytes ?? storageBytesForItem(item);
  }
  return source.filter((item) => !removed.has(item.id));
}

export function fingerprintImageBytes(width: number, height: number, bytes: Buffer): string {
  let hash = 0x811c9dc5;
  const len = bytes.length;
  const step = Math.max(1, Math.floor(len / 4096));
  for (let i = 0; i < len; i += step) {
    hash ^= bytes[i];
    hash = Math.imul(hash, 0x01000193);
  }
  return `${width}x${height}:${len}:${(hash >>> 0).toString(16)}`;
}
