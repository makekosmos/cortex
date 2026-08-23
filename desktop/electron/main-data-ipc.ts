import { BrowserWindow, dialog } from "electron";
import type { ArkClient } from "@kosmos/ark";
import type { SearchResult } from "../shared/ipc-types";
import { safeHandle } from "./ipc-safe";
import { keplerLog } from "./logging";
import { registerMainDataSettingsIpc } from "./main-data-ipc-settings";

interface MainDataIpcOptions {
  awaitArkReady(): Promise<ArkClient>;
  getArkClient(): ArkClient | null;
}
type ArkRequest = Parameters<ArkClient["invokeOperation"]>[0];
type ArkRequestParams = Omit<ArkRequest, "operation">;

export function registerMainDataIpc(options: MainDataIpcOptions): void {
  const { awaitArkReady, getArkClient } = options;

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
    async (_e, operation: string, params?: ArkRequestParams) => {
      if (typeof operation !== "string" || operation.length === 0) {
        throw new Error("kepler:ark:request: operation must be a non-empty string");
      }
      const client = await awaitArkReady();
      const req: ArkRequest = params ? { operation, ...params } : { operation };
      return client.invokeOperation(req);
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

  safeHandle("kepler:file-index:pick-root", async (e): Promise<string | null> => {
    if (process.env.KOSMOS_HEADLESS === "1" || process.env.KOSMOS_TEST_MODE === "1") return null;
    const win = BrowserWindow.fromWebContents(e.sender);
    const result = win
      ? await dialog.showOpenDialog(win, {
          title: "Выберите папку для индексации",
          properties: ["openDirectory", "createDirectory"],
        })
      : await dialog.showOpenDialog({
          title: "Выберите папку для индексации",
          properties: ["openDirectory", "createDirectory"],
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

  registerMainDataSettingsIpc();
}
