import { existsSync, readFileSync } from "node:fs";
import { mkdir, rename, writeFile } from "node:fs/promises";
import path from "node:path";
import type { ClipboardHistoryItem } from "../shared/ipc-types";
import {
  classifyClipboardText,
  defaultClipboardHistorySettings,
  type ClipboardHistoryStore,
  isItem,
  normalizeItem,
  normalizeNextId,
  normalizeSettings,
  normalizeSource,
  normalizeSourceIcon,
  pruneClipboardHistoryItems,
  totalStorageBytes,
  withStorageBytes,
  type ClipboardHistoryRecordMetadata,
  type PersistedClipboardHistory,
} from "./clipboard-history-store-helpers";

export function createClipboardHistoryStore(options?: {
  maxItems?: number;
  maxItemBytes?: number;
  now?: () => number;
  storagePath?: string;
}): ClipboardHistoryStore {
  const maxItems = options?.maxItems ?? 10_000;
  const maxItemBytes = options?.maxItemBytes ?? 10 * 1024 * 1024;
  const now = options?.now ?? Date.now;
  const storagePath = options?.storagePath;
  let settings = defaultClipboardHistorySettings();
  let items: ClipboardHistoryItem[] = [];
  let nextId = 1;
  let saveTimer: ReturnType<typeof setTimeout> | null = null;

  hydrate();
  if (items.length > 0) commit({ forceSave: false, pruneNow: now() });

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
    const item = withStorageBytes({
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
    } as ClipboardHistoryItem);
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
    const item = withStorageBytes({
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
    } as ClipboardHistoryItem);
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
    const item = withStorageBytes({
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
    } as ClipboardHistoryItem);
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
    const prevLength = items.length;
    items = pruneClipboardHistoryItems(items, settings, maxItems, opts?.pruneNow ?? now());
    if (opts?.forceSave === false && items.length === prevLength) return;
    scheduleSave();
  }

  function scheduleSave(): void {
    if (!storagePath || saveTimer) return;
    saveTimer = setTimeout(() => {
      saveTimer = null;
      void save();
    }, 400);
  }

  function flush(): Promise<void> {
    if (saveTimer) {
      clearTimeout(saveTimer);
      saveTimer = null;
    }
    return save();
  }

  function save(): Promise<void> {
    if (!storagePath) return Promise.resolve();
    const payload: PersistedClipboardHistory = {
      version: 1,
      nextId,
      settings,
      items,
    };
    return mkdir(path.dirname(storagePath), { recursive: true })
      .then(() => writeFile(`${storagePath}.tmp`, JSON.stringify(payload, null, 2), "utf8"))
      .then(() => rename(`${storagePath}.tmp`, storagePath))
      .catch((e) => {
        console.error("[clipboard-history] failed to persist history:", e);
      });
  }

  function find(id: string): ClipboardHistoryItem | null {
    return items.find((item) => item.id === id) ?? null;
  }

  return {
    record,
    recordImage,
    recordFile,
    list: () => {
      commit();
      return [...items];
    },
    find,
    updateTimestamp: (id) => {
      const item = find(id);
      return item ? touch(item) : null;
    },
    togglePin: (id) => {
      const item = find(id);
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
    flush,
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
