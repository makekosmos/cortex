// Eden e2e helpers.
//
// Переиспользуемые куски для Eden spec'ов:
//   - openEden(app) — launcher warmup + invoke `eden:open` → ждём extension window.
//   - createEdenNote(edenWindow, {id?, title?, content?}) — saveEntry через shim
//     с дефолтами note_obj. Возвращает id.
//   - getProseMirrorJSON(edenWindow) — best-effort извлечение editor.getJSON()
//     через DOM-spelunking (TipTap Vue node view → __vueParentComponent →
//     до Editor instance). Если не получится, fallback на DOM-структуру через
//     getProseMirrorStructure().
//   - getProseMirrorStructure(edenWindow) — простая DOM-сериализация:
//     возвращает { lastChildTag, lastChildIsEmpty, childCount, children: [...] }.
//
// См. также `tests/e2e/eden.spec.ts` — старый source паттернов до экстракции.
//
// ВАЖНО: эти helpers НЕ override'ят `KOSMOS_HEADLESS=1` и НЕ передают
// `KOSMOS_DATA_DIR` явно — launchKepler делает это сам.

import { expect, type Page } from "@playwright/test";
import type { ElectronApplication } from "playwright";
import { waitForBackendReady } from "./wait";

interface EdenApi {
  saveEntry: (e: unknown) => Promise<{ ok: boolean; entryId?: string }>;
  listEntries: () => Promise<
    Array<{
      id: string;
      title: string;
      content_json?: string;
      type_id?: string | null;
      deleted_at?: number | null;
    }>
  >;
  loadEntry: (id: string) => Promise<{ id: string; title: string; content_json?: string } | null>;
  deleteEntry: (id: string) => Promise<unknown>;
  createTask?: (sourceNoteId: string, title?: string, explicitId?: string) => Promise<string>;
}

/**
 * Открыть Eden extension window через command bus `eden:open`.
 * Warmup ≥2.5s до invoke + waitForLoadState/timeout после open.
 */
export async function openEden(app: ElectronApplication): Promise<Page> {
  const launcher = await app.firstWindow();
  await launcher.waitForLoadState("domcontentloaded");
  // Phase 1 determinism: ждём ArkClient handshake вместо waitForTimeout(2500).
  // Если ready — резолвится мгновенно. См. tests/e2e/helpers/wait.ts.
  await waitForBackendReady(launcher);

  const triggered = await launcher.evaluate(async () => {
    if (typeof window.kepler?.commands?.invoke !== "function") return "no-api";
    try {
      await window.kepler.commands.invoke("eden:open");
      return "ok";
    } catch (e) {
      return "throw:" + (e instanceof Error ? e.message : String(e));
    }
  });
  expect(triggered, `eden:open status: ${triggered}`).toBe("ok");

  const edenWindow = await app.waitForEvent("window", { timeout: 10_000 });
  await edenWindow.waitForLoadState("domcontentloaded");
  // Eden extension preload exposes `kepler.__test` под KOSMOS_TEST_MODE.
  // Ждём что ArkClient видит свой extension window (handshake уже был, но
  // extension загружается чуть позже).
  await waitForBackendReady(edenWindow);
  return edenWindow;
}

export interface CreateNoteOpts {
  id?: string;
  title?: string;
  /** TipTap doc JSON. Default — single empty paragraph. */
  contentJson?: unknown;
  typeId?: string;
}

/**
 * Создать note_obj entry через `window.api.saveEntry`. Возвращает id.
 */
export async function createEdenNote(edenWindow: Page, opts: CreateNoteOpts = {}): Promise<string> {
  const id = opts.id ?? `eden-e2e-${Date.now()}-${Math.floor(Math.random() * 1e6)}`;
  const title = opts.title ?? `e2e-note-${id.slice(-6)}`;
  const contentJson = opts.contentJson ?? { type: "doc", content: [{ type: "paragraph" }] };
  const typeId = opts.typeId ?? "note_obj";

  const result = await edenWindow.evaluate(
    async (payload) => {
      const api = (window as unknown as { api: EdenApi }).api;
      try {
        const r = await api.saveEntry({
          id: payload.id,
          title: payload.title,
          content_json: JSON.stringify(payload.contentJson),
          created_at: Date.now(),
          updated_at: Date.now(),
          folder_id: null,
          type_id: payload.typeId,
          header_layout: "default",
          header_props_json: "{}",
          schema_version: 1,
          deleted_at: null,
        });
        return { ok: true, r };
      } catch (e) {
        return { ok: false, error: e instanceof Error ? e.message : String(e) };
      }
    },
    { id, title, contentJson, typeId },
  );

  expect(result.ok, `createEdenNote saveEntry should succeed: ${JSON.stringify(result)}`).toBe(
    true,
  );
  return id;
}

/**
 * Reload Eden on its home feed, preload the full entry, and navigate through
 * the existing store action. Returns true when the TipTap editor mounts.
 *
 * The public helper name stays stable for existing e2e specs even though
 * cross-launch last-entry restoration is intentionally no longer supported.
 * Preloading preserves these editor-focused specs' original full-body mount
 * contract; the Everything visual spec covers the real summary-card path.
 */
export async function openNoteViaReload(edenWindow: Page, entryId: string): Promise<boolean> {
  await edenWindow.reload();
  await edenWindow.waitForLoadState("domcontentloaded");
  try {
    await edenWindow.waitForFunction(() => typeof window.api?.loadEntry === "function", null, {
      timeout: 10_000,
    });
    const opened = await edenWindow.evaluate(async (id) => {
      const fullEntry = await window.api.loadEntry(id);
      if (!fullEntry) return false;

      type EdenTestStore = {
        entries: Entry[];
        navigateTo: (entryId: string) => Promise<void>;
      };
      type PiniaLike = { _s?: Map<string, EdenTestStore> };
      const root = document.querySelector("#root") as
        | (Element & {
            __vue_app__?: {
              _context?: { provides?: Record<PropertyKey, unknown> };
            };
          })
        | null;
      const provides = root?.__vue_app__?._context?.provides;
      const pinia = provides
        ? Reflect.ownKeys(provides)
            .map((key) => provides[key])
            .find(
              (candidate): candidate is PiniaLike =>
                !!candidate &&
                typeof candidate === "object" &&
                (candidate as PiniaLike)._s instanceof Map,
            )
        : null;
      const store = pinia?._s?.get("eden");
      if (!store) throw new Error("Eden Pinia store not found");
      const loadedEntry = { ...fullEntry, content_loaded: true };
      const existingIndex = store.entries.findIndex((entry) => entry.id === id);
      store.entries =
        existingIndex >= 0
          ? store.entries.map((entry, index) => (index === existingIndex ? loadedEntry : entry))
          : [loadedEntry, ...store.entries];
      await store.navigateTo(id);
      return true;
    }, entryId);
    if (!opened) return false;

    await edenWindow.locator(".ProseMirror").waitFor({ state: "visible", timeout: 10_000 });
    // Preserve the helper's previous settle contract for TipTap transactions
    // and callers that reload again immediately after debounced autosave.
    await edenWindow.waitForTimeout(2500);
    return true;
  } catch (error) {
    console.warn(`[eden helpers] openNoteViaReload(${entryId}) failed:`, error);
    return false;
  }
}

/**
 * Достать `editor.getJSON()` из активного TipTap editor.
 *
 * Стратегия: ProseMirror DOM → element[pmViewDesc].node — это сам ProseMirror
 * Node корня, но не editor instance. Editor instance прячется на Vue компоненте
 * Editor.vue. Идём через `__vue_app__` глобал и ищем компонент с polish
 * `editor` ref. В случае неудачи возвращаем null — caller fallback'ает на
 * getProseMirrorStructure.
 */
export async function getProseMirrorJSON(edenWindow: Page): Promise<unknown | null> {
  return await edenWindow.evaluate(() => {
    const pm = document.querySelector(".ProseMirror") as
      | (HTMLElement & { pmViewDesc?: { node?: unknown } })
      | null;
    if (!pm) return null;
    // pmViewDesc.node — корневой ProseMirror Node. У него .toJSON() — публичный.
    const desc = (pm as unknown as { pmViewDesc?: { node?: { toJSON: () => unknown } } })
      .pmViewDesc;
    if (desc?.node?.toJSON) {
      try {
        return desc.node.toJSON();
      } catch (e) {
        return { __error: e instanceof Error ? e.message : String(e) };
      }
    }
    return null;
  });
}

/**
 * Fallback DOM-сериализация структуры ProseMirror — возвращает массив
 * top-level children с tag и textContent. lastChildEmpty=true если последний
 * child — paragraph без видимого текста (только <br> или пустота).
 */
export async function getProseMirrorStructure(edenWindow: Page): Promise<{
  childCount: number;
  children: Array<{ tag: string; classes: string; text: string; isEmpty: boolean }>;
  lastChildTag: string | null;
  lastChildIsEmpty: boolean;
} | null> {
  return await edenWindow.evaluate(() => {
    const pm = document.querySelector(".ProseMirror");
    if (!pm) return null;
    const children = Array.from(pm.children).map((el) => {
      const text = el.textContent ?? "";
      // Пустой paragraph в PM рендерится как <p><br class="ProseMirror-trailingBreak"></p>
      // textContent === "" но всё равно node существует.
      const onlyBr = el.children.length === 1 && el.children[0].tagName === "BR";
      const isEmpty = text.trim() === "" && (el.children.length === 0 || onlyBr);
      return {
        tag: el.tagName.toLowerCase(),
        classes: el.className ?? "",
        text: text.trim(),
        isEmpty,
      };
    });
    return {
      childCount: children.length,
      children,
      lastChildTag: children.length > 0 ? children[children.length - 1].tag : null,
      lastChildIsEmpty: children.length > 0 ? children[children.length - 1].isEmpty : false,
    };
  });
}

/**
 * Soft-delete entry через shim. Не throw'ит на ошибках.
 */
export async function deleteEdenEntry(edenWindow: Page, entryId: string): Promise<void> {
  await edenWindow.evaluate(async (id) => {
    try {
      await (
        window as unknown as { api: { deleteEntry: (id: string) => Promise<unknown> } }
      ).api.deleteEntry(id);
    } catch (e) {
      console.warn("[eden helpers] deleteEntry failed:", e);
    }
  }, entryId);
}
