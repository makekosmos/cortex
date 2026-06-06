import { existsSync, readFileSync } from "node:fs";
import { mkdir, rename, writeFile } from "node:fs/promises";
import path from "node:path";
import type {
  ClipboardHistoryItem,
  ClipboardHistorySettings,
  ClipboardHistorySettingsPatch,
  ClipboardHistoryStats,
} from "../shared/ipc-types";

const DEFAULT_MAX_ITEMS = 10_000;
const DEFAULT_RETENTION_DAYS = 30;
const DEFAULT_MAX_BYTES = 64 * 1024 * 1024;
const DEFAULT_MAX_ITEM_BYTES = 10 * 1024 * 1024;
const PREVIEW_LIMIT = 180;
const DAY_MS = 24 * 60 * 60 * 1000;
const SAVE_DEBOUNCE_MS = 400;

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
  stats(): ClipboardHistoryStats;
  pruneNow(): void;
}

export interface ClipboardHistoryRecordMetadata {
  source?: string;
  sourceIcon?: string;
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

interface PersistedClipboardHistory {
  version: 1;
  nextId: number;
  settings: ClipboardHistorySettings;
  items: ClipboardHistoryItem[];
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

export function createClipboardHistoryStore(options?: {
  maxItems?: number;
  maxItemBytes?: number;
  now?: () => number;
  storagePath?: string;
}): ClipboardHistoryStore {
  const maxItems = options?.maxItems ?? DEFAULT_MAX_ITEMS;
  const maxItemBytes = options?.maxItemBytes ?? DEFAULT_MAX_ITEM_BYTES;
  const now = options?.now ?? Date.now;
  const storagePath = options?.storagePath;
  let settings = defaultClipboardHistorySettings();
  let items: ClipboardHistoryItem[] = [];
  let nextId = 1;
  let saveTimer: ReturnType<typeof setTimeout> | null = null;

  hydrate();
  if (items.length > 0) {
    commit({ forceSave: false, pruneNow: now() });
  }

  function record(
    text: string,
    metadata: ClipboardHistoryRecordMetadata = {},
  ): ClipboardHistoryItem | null {
    const classified = classifyClipboardText(text);
    if (!classified) return null;

    const existing = items.find(
      (item) =>
        (item.kind === "text" || item.kind === "link" || item.kind === "color") &&
        item.text === classified.text,
    );
    if (existing) return touch(existing);

    const timestamp = now();
    const item: ClipboardHistoryItem = withStorageBytes({
      id: `clip-${nextId++}`,
      kind: classified.kind,
      text: classified.text,
      preview: classified.preview,
      createdAt: timestamp,
      updatedAt: timestamp,
      charCount: classified.text.length,
      pinned: false,
      searchText: classified.searchText,
      source: normalizeSource(metadata.source),
      sourceIcon: normalizeSourceIcon(metadata.sourceIcon),
      url: classified.url,
      color: classified.color,
    });
    if ((item.storageBytes ?? 0) > maxItemBytes) return null;
    insert(item, timestamp);
    return item;
  }

  function recordImage(args: {
    dataUrl: string;
    width: number;
    height: number;
    mimeType?: string;
    source?: string;
    sourceIcon?: string;
  }): ClipboardHistoryItem | null {
    if (!args.dataUrl || args.width <= 0 || args.height <= 0) return null;

    const existing = items.find(
      (item) => item.kind === "image" && item.imageDataUrl === args.dataUrl,
    );
    if (existing) return touch(existing);

    const timestamp = now();
    const item: ClipboardHistoryItem = withStorageBytes({
      id: `clip-${nextId++}`,
      kind: "image",
      text: "",
      preview: `Изображение ${args.width}×${args.height}`,
      createdAt: timestamp,
      updatedAt: timestamp,
      charCount: 0,
      pinned: false,
      searchText: `изображение image ${args.width} ${args.height}`,
      source: normalizeSource(args.source),
      sourceIcon: normalizeSourceIcon(args.sourceIcon),
      imageDataUrl: args.dataUrl,
      width: args.width,
      height: args.height,
      mimeType: args.mimeType,
    });
    if ((item.storageBytes ?? 0) > maxItemBytes) return null;
    insert(item, timestamp);
    return item;
  }

  function recordFile(
    filePath: string,
    metadata: ClipboardHistoryRecordMetadata = {},
  ): ClipboardHistoryItem | null {
    const normalized = filePath.trim();
    if (!normalized) return null;
    const existing = items.find((item) => item.kind === "file" && item.filePath === normalized);
    if (existing) return touch(existing);

    const fileName = normalized.split(/[\\/]/).filter(Boolean).at(-1) ?? normalized;
    const timestamp = now();
    const item: ClipboardHistoryItem = withStorageBytes({
      id: `clip-${nextId++}`,
      kind: "file",
      text: normalized,
      preview: fileName,
      createdAt: timestamp,
      updatedAt: timestamp,
      charCount: normalized.length,
      pinned: false,
      searchText: `${fileName} ${normalized}`,
      source: normalizeSource(metadata.source),
      sourceIcon: normalizeSourceIcon(metadata.sourceIcon),
      filePath: normalized,
      fileName,
    });
    if ((item.storageBytes ?? 0) > maxItemBytes) return null;
    insert(item, timestamp);
    return item;
  }

  function touch(existing: ClipboardHistoryItem): ClipboardHistoryItem {
    const timestamp = now();
    const updated = withStorageBytes({ ...existing, updatedAt: timestamp });
    items = [updated, ...items.filter((item) => item.id !== existing.id)];
    commit({ pruneNow: timestamp });
    return updated;
  }

  function insert(item: ClipboardHistoryItem, pruneNow: number): void {
    items = [item, ...items];
    commit({ pruneNow });
  }

  function hydrate(): void {
    if (!storagePath || !existsSync(storagePath)) return;
    try {
      const parsed = JSON.parse(
        readFileSync(storagePath, "utf8"),
      ) as Partial<PersistedClipboardHistory>;
      settings = normalizeSettings(parsed.settings);
      items = Array.isArray(parsed.items) ? parsed.items.map(normalizeItem).filter(isItem) : [];
      nextId = normalizeNextId(parsed.nextId, items);
    } catch (e) {
      console.error("[clipboard-history] failed to read persisted history:", e);
      settings = defaultClipboardHistorySettings();
      items = [];
      nextId = 1;
    }
  }

  function commit(opts?: { forceSave?: boolean; pruneNow?: number }): void {
    // storageBytes уже проставлен на каждом элементе при создании/изменении —
    // повторно сериализовать всю историю здесь не нужно (раньше тройной
    // JSON.stringify на каждое копирование вешал main-процесс).
    const prevLength = items.length;
    items = prune(sortItems(items), opts?.pruneNow ?? now());
    if (opts?.forceSave === false && items.length === prevLength) return;
    scheduleSave();
  }

  function scheduleSave(): void {
    if (!storagePath || saveTimer) return;
    saveTimer = setTimeout(() => {
      saveTimer = null;
      void save();
    }, SAVE_DEBOUNCE_MS);
  }

  function sortItems(source: ClipboardHistoryItem[]): ClipboardHistoryItem[] {
    return [...source].sort(
      (a, b) => Number(b.pinned) - Number(a.pinned) || b.updatedAt - a.updatedAt,
    );
  }

  function prune(source: ClipboardHistoryItem[], pruneNow: number): ClipboardHistoryItem[] {
    const retentionDays = Math.floor(settings.retentionDays);
    const cutoff = retentionDays > 0 ? pruneNow - retentionDays * DAY_MS : Number.NEGATIVE_INFINITY;
    let next = source.filter((item) => item.pinned || item.updatedAt >= cutoff);

    const pinned = next.filter((item) => item.pinned);
    const unpinned = next
      .filter((item) => !item.pinned)
      .slice(0, Math.max(0, maxItems - pinned.length));
    next = sortItems([...pinned, ...unpinned]);

    if (settings.maxBytes > 0) {
      next = pruneBySize(next, settings.maxBytes);
    }
    return next;
  }

  function pruneBySize(source: ClipboardHistoryItem[], maxBytes: number): ClipboardHistoryItem[] {
    let total = totalStorageBytes(source);
    if (total <= maxBytes) return source;

    const removable = source
      .filter((item) => !item.pinned)
      .sort((a, b) => a.updatedAt - b.updatedAt);
    const removed = new Set<string>();
    for (const item of removable) {
      if (total <= maxBytes) break;
      removed.add(item.id);
      total -= item.storageBytes ?? storageBytesForItem(item);
    }
    return source.filter((item) => !removed.has(item.id));
  }

  async function save(): Promise<void> {
    if (!storagePath) return;
    const payload: PersistedClipboardHistory = {
      version: 1,
      nextId,
      settings,
      items,
    };
    try {
      await mkdir(path.dirname(storagePath), { recursive: true });
      const tmp = `${storagePath}.tmp`;
      await writeFile(tmp, JSON.stringify(payload, null, 2), "utf8");
      await rename(tmp, storagePath);
    } catch (e) {
      console.error("[clipboard-history] failed to persist history:", e);
    }
  }

  return {
    record,
    recordImage,
    recordFile,
    list: () => {
      commit();
      return [...items];
    },
    find: (id) => items.find((item) => item.id === id) ?? null,
    updateTimestamp: (id) => {
      const item = items.find((candidate) => candidate.id === id);
      return item ? touch(item) : null;
    },
    togglePin: (id) => {
      const item = items.find((candidate) => candidate.id === id);
      if (!item) return null;
      const timestamp = now();
      const updated = withStorageBytes({ ...item, pinned: !item.pinned, updatedAt: timestamp });
      items = [updated, ...items.filter((candidate) => candidate.id !== id)];
      commit({ pruneNow: timestamp });
      return updated;
    },
    remove: (id) => {
      const before = items.length;
      items = items.filter((item) => item.id !== id);
      const removed = items.length !== before;
      if (removed) commit();
      return removed;
    },
    clearUnpinned: () => {
      items = items.filter((item) => item.pinned);
      commit();
    },
    clear: () => {
      items = items.filter((item) => item.pinned);
      commit();
    },
    clearAll: () => {
      items = [];
      commit();
    },
    settings: () => ({ ...settings }),
    updateSettings: (patch) => {
      settings = normalizeSettings({ ...settings, ...patch });
      commit({ forceSave: true });
      return { ...settings };
    },
    stats: () => {
      commit();
      return {
        itemCount: items.length,
        pinnedCount: items.filter((item) => item.pinned).length,
        storageBytes: totalStorageBytes(items),
        oldestItemAt: items.length > 0 ? Math.min(...items.map((item) => item.updatedAt)) : null,
      };
    },
    pruneNow: () => commit({ forceSave: true }),
  };
}

function normalizeClipboardText(text: string): string {
  return text.replace(/\r\n/g, "\n").trim();
}

function normalizeSource(source: string | undefined): string | undefined {
  const value = source?.trim();
  return value ? value : undefined;
}

function normalizeSourceIcon(sourceIcon: string | undefined): string | undefined {
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

function normalizeSettings(input: unknown): ClipboardHistorySettings {
  const raw = (input ?? {}) as Partial<ClipboardHistorySettings>;
  const defaults = defaultClipboardHistorySettings();
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

function normalizeNextId(input: unknown, items: ClipboardHistoryItem[]): number {
  if (typeof input === "number" && Number.isFinite(input) && input > 0) return Math.floor(input);
  const maxId = items.reduce((max, item) => {
    const parsed = item.id.match(/^clip-(\d+)$/)?.[1];
    return parsed ? Math.max(max, Number(parsed)) : max;
  }, 0);
  return maxId + 1;
}

function normalizeItem(input: unknown): ClipboardHistoryItem {
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

function isItem(item: ClipboardHistoryItem): boolean {
  return Boolean(item.id && item.preview !== undefined && item.searchText !== undefined);
}

function withStorageBytes(item: ClipboardHistoryItem): ClipboardHistoryItem {
  const { storageBytes: _storageBytes, ...rest } = item;
  return {
    ...rest,
    storageBytes: storageBytesForItem(rest),
  };
}

function storageBytesForItem(item: Omit<ClipboardHistoryItem, "storageBytes">): number {
  return Buffer.byteLength(JSON.stringify(item), "utf8");
}

function totalStorageBytes(items: ClipboardHistoryItem[]): number {
  return items.reduce((sum, item) => sum + (item.storageBytes ?? storageBytesForItem(item)), 0);
}

/**
 * Быстрый отпечаток raw-битмапа изображения (FNV-1a по сэмплированным байтам).
 * Нужен, чтобы определять смену картинки в буфере БЕЗ дорогого PNG-кодирования
 * (`toDataURL`) на каждом тике поллинга — кодируем только когда отпечаток сменился.
 */
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
