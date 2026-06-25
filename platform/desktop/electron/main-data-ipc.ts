import { BrowserWindow, dialog } from "electron";
import type { ArkClient } from "@kosmos/ark";
import type { SearchResult } from "../shared/ipc-types";
import { safeHandle } from "./ipc-safe";
import { keplerLog } from "./logging";
import { registerMainDataSettingsIpc } from "./main-data-ipc-settings";

interface MainDataIpcOptions {
  awaitArkReady(): Promise<ArkClient>;
  getArkClient(): ArkClient | null;
  broadcastSettingsSyncUpdated(): void;
}

export function registerMainDataIpc(options: MainDataIpcOptions): void {
  const { awaitArkReady, broadcastSettingsSyncUpdated, getArkClient } = options;

  safeHandle("kepler:search:query", async (_e, text: string): Promise<SearchResult[]> => {
    const arkClient = getArkClient();
    if (!arkClient || !text.trim()) return [];
    try {
      const hits = await arkClient.objects.search(text);
      if (hits.length === 0) return [];
      const ids = Array.from(new Set(hits.map((h) => h.entryId))).slice(0, 8);
      const records = await arkClient.objects.getMany(ids);
      const recordById = new Map(records.map((r) => [r.id, r]));
      const out: SearchResult[] = [];
      const seen = new Set<string>();
      for (const h of hits) {
        if (seen.has(h.entryId)) continue;
        seen.add(h.entryId);
        const rec = recordById.get(h.entryId);
        if (!rec) continue;
        out.push({
          id: rec.id,
          title: rec.title && rec.title.length > 0 ? rec.title : rec.id,
          type_id: rec.typeId,
          snippet: h.text,
        });
        if (out.length >= 8) break;
      }
      return out;
    } catch (e) {
      keplerLog.warn("search", "ARK search failed", { err: String(e) });
      return [];
    }
  });

  safeHandle(
    "kepler:ark:request",
    async (_e, operation: string, params?: Record<string, unknown>) => {
      if (typeof operation !== "string" || operation.length === 0) {
        throw new Error("kepler:ark:request: operation must be a non-empty string");
      }
      const client = await awaitArkReady();
      const req: Record<string, unknown> = { operation, ...params };
      return client.invokeOperation(req as { operation: string; [key: string]: unknown });
    },
  );

  safeHandle("kepler:export:list", async () => {
    const client = await awaitArkReady();
    // Backend returns `{ converters: [...] }`. Renderer ожидает плоский массив.
    const resp = (await client.invokeOperation({ operation: "export.list" })) as
      | { converters?: unknown }
      | unknown[]
      | null;
    if (Array.isArray(resp)) return resp;
    if (
      resp &&
      typeof resp === "object" &&
      Array.isArray((resp as { converters?: unknown }).converters)
    ) {
      return (resp as { converters: unknown[] }).converters;
    }
    return [];
  });

  safeHandle(
    "kepler:export:run",
    async (_e, args: { converter_id: string; format: string; dest_dir: string }) => {
      if (!args || typeof args.converter_id !== "string") {
        throw new Error("kepler:export:run: invalid args");
      }
      const client = await awaitArkReady();
      return client.invokeOperation({
        operation: "export.run",
        converter_id: args.converter_id,
        format: args.format,
        dest_dir: args.dest_dir,
      });
    },
  );

  safeHandle("kepler:export:pickDir", async (e): Promise<string | null> => {
    const win = BrowserWindow.fromWebContents(e.sender);
    const result = win
      ? await dialog.showOpenDialog(win, {
          title: "Выберите папку для экспорта",
          properties: ["openDirectory", "createDirectory"],
        })
      : await dialog.showOpenDialog({
          title: "Выберите папку для экспорта",
          properties: ["openDirectory", "createDirectory"],
        });
    if (result.canceled || result.filePaths.length === 0) return null;
    return result.filePaths[0];
  });

  safeHandle("kepler:file-search:settings:get", async () => {
    const client = await awaitArkReady();
    return client.invokeOperation({ operation: "file_index.settings_get" });
  });

  safeHandle("kepler:file-search:diagnostics", async () => {
    const client = await awaitArkReady();
    return client.invokeOperation({ operation: "file_index.diagnostics" });
  });

  safeHandle("kepler:file-search:estimate-root", async (_e, pathInput: string) => {
    if (typeof pathInput !== "string" || pathInput.trim().length === 0) {
      throw new Error("kepler:file-search:estimate-root invalid path");
    }
    const client = await awaitArkReady();
    return client.invokeOperation({
      operation: "file_index.estimate_root",
      path: pathInput,
    });
  });

  safeHandle("kepler:file-search:settings:set", async (_e, patch: Record<string, unknown>) => {
    // Regression H9 (2026-05-24): strict allowlist of bool fields.
    const ALLOWED_BOOL_FIELDS = [
      "enabled",
      "exclude_noisy_folders",
      "respect_gitignore",
      "include_hidden",
      "ntfs_accelerated",
    ] as const;
    const sanitized: Record<string, boolean> = {};
    if (patch && typeof patch === "object") {
      for (const key of ALLOWED_BOOL_FIELDS) {
        const value = (patch as Record<string, unknown>)[key];
        if (typeof value === "boolean") sanitized[key] = value;
      }
    }
    const client = await awaitArkReady();
    await client.invokeOperation({
      operation: "file_index.settings_set",
      ...sanitized,
    });
  });

  safeHandle("kepler:file-search:scope:add", async (_e, path: string) => {
    if (typeof path !== "string" || path.trim().length === 0) {
      throw new Error("kepler:file-search:scope:add invalid path");
    }
    const safety = isLikelyUnsafeScope(path);
    if (!safety.ok) {
      throw new Error(safety.reason);
    }
    const client = await awaitArkReady();
    await client.invokeOperation({ operation: "file_index.scope_add", path });
  });

  safeHandle("kepler:file-search:scope:remove", async (_e, path: string) => {
    if (typeof path !== "string" || path.trim().length === 0) {
      throw new Error("kepler:file-search:scope:remove invalid path");
    }
    const client = await awaitArkReady();
    await client.invokeOperation({ operation: "file_index.scope_remove", path });
  });

  safeHandle("kepler:file-search:ignore:add", async (_e, pattern: string) => {
    if (typeof pattern !== "string" || pattern.trim().length === 0) {
      throw new Error("kepler:file-search:ignore:add invalid pattern");
    }
    const client = await awaitArkReady();
    await client.invokeOperation({ operation: "file_index.ignore_add", pattern });
  });

  safeHandle("kepler:file-search:ignore:remove", async (_e, pattern: string) => {
    if (typeof pattern !== "string" || pattern.trim().length === 0) {
      throw new Error("kepler:file-search:ignore:remove invalid pattern");
    }
    const client = await awaitArkReady();
    await client.invokeOperation({
      operation: "file_index.ignore_remove",
      pattern,
    });
  });

  safeHandle("kepler:file-search:rescan", async () => {
    const client = await awaitArkReady();
    await client.invokeOperation({ operation: "file_index.rescan" });
  });

  safeHandle("kepler:file-search:clear-cache", async () => {
    const client = await awaitArkReady();
    await client.invokeOperation({ operation: "file_index.clear_cache" });
  });

  safeHandle("kepler:file-search:pickScope", async (e): Promise<string | null> => {
    // Regression M9 (2026-05-24): native dialog would hang e2e under
    // KOSMOS_HEADLESS=1. Skip silently in headless mode.
    if (process.env.KOSMOS_HEADLESS === "1") return null;
    const win = BrowserWindow.fromWebContents(e.sender);
    const result = win
      ? await dialog.showOpenDialog(win, {
          title: "Добавить папку поиска",
          properties: ["openDirectory"],
        })
      : await dialog.showOpenDialog({
          title: "Добавить папку поиска",
          properties: ["openDirectory"],
        });
    if (result.canceled || result.filePaths.length === 0) return null;
    return result.filePaths[0];
  });

  safeHandle("kepler:objects:listRecent", async (_e, limit?: number): Promise<SearchResult[]> => {
    const arkClient = getArkClient();
    if (!arkClient) return [];
    const cap = typeof limit === "number" && limit > 0 ? Math.min(limit, 500) : 200;
    try {
      const records = await arkClient.objects.list();
      const sorted = records
        .filter((r) => !r.deletedAt)
        .sort((a, b) => (a.updatedAt < b.updatedAt ? 1 : -1))
        .slice(0, cap);
      return sorted.map((r) => ({
        id: r.id,
        title: r.title && r.title.length > 0 ? r.title : r.id,
        type_id: r.typeId,
      }));
    } catch (e) {
      keplerLog.warn("objects", "objects.list failed", { err: String(e) });
      return [];
    }
  });

  registerMainDataSettingsIpc({
    getArkClient,
    broadcastSettingsSyncUpdated,
  });
}

function isLikelyUnsafeScope(raw: string): { ok: true } | { ok: false; reason: string } {
  // Regression H8 (2026-05-24): refuse UNC and warn-block on whole-drive roots.
  const trimmed = raw.trim();
  if (trimmed.startsWith("\\\\") || trimmed.startsWith("//")) {
    return { ok: false, reason: "Сетевые пути (UNC) пока не поддерживаются" };
  }
  return { ok: true };
}
