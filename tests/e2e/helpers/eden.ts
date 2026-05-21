// Eden e2e helpers.
//
// Переиспользуемые куски для Eden spec'ов:
//   - openEden(app) — launcher warmup + invoke `eden:open` → ждём extension window.
//   - createEdenNote(edenWindow, {id?, title?, content?}) — saveEntry через shim
//     с дефолтами note_obj. Возвращает id.
//   - openEdenNote(edenWindow, id) — навигация на заметку через клик в sidebar
//     по `[data-entry-id="<id>"]` (или fallback по title-тексту). Если не найдена —
//     возвращает "not-found".
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

export interface EdenApi {
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
 * Установить `eden:nav:lastEntryId` в localStorage у Eden window — initApp
 * подхватит при следующем mount. Использовать ПЕРЕД close+reopen Eden чтобы
 * нужная заметка автоматически открылась.
 */
export async function setEdenLastEntry(edenWindow: Page, entryId: string): Promise<void> {
  await edenWindow.evaluate((id) => {
    try {
      window.localStorage.setItem("eden:nav:lastEntryId", id);
    } catch {
      /* ignore */
    }
  }, entryId);
}

/**
 * Навигация на заметку через DOM-click в sidebar. Возвращает строку-статус:
 * "clicked-sidebar" | "clicked-by-text" | "not-found".
 *
 * Не делает waitForTimeout после клика — caller сам ждёт rendering.
 */
/**
 * Hardest-fix navigation: ставим entryId в localStorage и reload'им
 * Eden window — initApp подхватит и откроет заметку. Возвращает true
 * если после reload в .ProseMirror отрендерилось содержимое (или
 * вообще ProseMirror смонтировался).
 *
 * Эта стратегия надёжнее DOM-click на sidebar (entries refresh ленивый,
 * sidebar item может ещё не отрендериться).
 */
export async function openNoteViaReload(edenWindow: Page, entryId: string): Promise<boolean> {
  await setEdenLastEntry(edenWindow, entryId);
  await edenWindow.reload();
  await edenWindow.waitForLoadState("domcontentloaded");
  // Дать initApp + shim install + Editor lazy chunk.
  await edenWindow.waitForTimeout(2500);
  return await edenWindow.evaluate(() => !!document.querySelector(".ProseMirror"));
}

export async function openEdenNote(
  edenWindow: Page,
  entryId: string,
  expectedTitle?: string,
): Promise<string> {
  // Sidebar item для recent entry рендерится с data-testid="recent-entry-<id>"
  // (см. EdenSidebar.vue::buildEntryItem + visuals/Sidebar.vue). Если note
  // только что создан и ещё не в `recentEntries.value` (refresh периодически
  // через subscribeObjectChanges) — может потребоваться wait.
  const status = await edenWindow.evaluate(
    async ({ id, title }) => {
      const selector = `[data-testid="recent-entry-${id}"]`;
      // Poll до 3s — entries.value обновляется через ARK object_upserted event.
      const start = Date.now();
      while (Date.now() - start < 3000) {
        const el = document.querySelector(selector) as HTMLElement | null;
        if (el) {
          el.click();
          return "clicked-sidebar";
        }
        await new Promise((r) => setTimeout(r, 150));
      }
      // Fallback: by-text. Точное совпадение текста в data-testid="kosmos-sidebar".
      if (title) {
        const sidebar = document.querySelector('[data-testid="kosmos-sidebar"]');
        if (sidebar) {
          const all = Array.from(sidebar.querySelectorAll<HTMLElement>("*"));
          const byText = all.find((el) => (el.textContent ?? "").trim() === title);
          if (byText) {
            byText.click();
            return "clicked-by-text";
          }
        }
      }
      return "not-found";
    },
    { id: entryId, title: expectedTitle },
  );
  return status;
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
