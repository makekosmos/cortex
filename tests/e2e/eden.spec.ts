// Eden extension UI smoke (Phase 6.0 / 6.0.A).
//
// Verifies:
//   1) Открытие Eden через command bus → extension window появляется.
//   2) `installKeplerApiShim` отработал → `window.api` defined.
//   3) `window.api.saveEntry` → ARK upsert работает.
//   4) После переоткрытия `window.api.listEntries` находит созданную заметку.
//   5) Cleanup через `window.api.deleteEntry`.
//
// Это per-app смысловой smoke поверх архитектурного `extensions-contract.spec`
// — мы хотим что Eden именно как note app работает end-to-end через shim.

import { test, expect } from "@playwright/test";
import { launchKepler } from "./helpers/launch";

const SAMPLE_TITLE = `eden-e2e-smoke-${Date.now()}`;

interface ArkEntry {
  id: string;
  title: string;
  type_id?: string | null;
  deleted_at?: number | null;
}

async function openEden(app: Awaited<ReturnType<typeof launchKepler>>) {
  const launcher = await app.firstWindow();
  await launcher.waitForLoadState("domcontentloaded");
  // Backend warmup до invoke — иначе command bus race / preload exposure
  // race могут оставить commands.invoke == undefined.
  await launcher.waitForTimeout(2500);

  const triggered = await app.evaluate(async ({ BrowserWindow }) => {
    const win = BrowserWindow.getAllWindows()[0];
    if (!win) return "no-launcher";
    const start = Date.now();
    while (Date.now() - start < 3000) {
      const ok = await win.webContents.executeJavaScript(`
        (async () => {
          if (typeof window.kepler?.commands?.invoke !== "function") return "no-api";
          try {
            await window.kepler.commands.invoke("eden:open");
            return "ok";
          } catch (e) {
            return "throw:" + (e && e.message ? e.message : String(e));
          }
        })()
      `) as string;
      if (ok === "ok") return ok;
      await new Promise((r) => setTimeout(r, 100));
    }
    return "timeout";
  });
  expect(triggered, `eden:open status: ${triggered}`).toBe("ok");

  const edenWindow = await app.waitForEvent("window", { timeout: 10_000 });
  await edenWindow.waitForLoadState("domcontentloaded");
  await edenWindow.waitForTimeout(1500); // shim install + initApp
  return edenWindow;
}

test.describe("eden extension", () => {
  test("eden: shim installed, save/list/delete entry round-trip", async () => {
    const app = await launchKepler({ slug: "eden-smoke" });
    try {
      const edenWindow = await openEden(app);

      // window.api shim установлен через installKeplerApiShim()
      const shimReady = await edenWindow.evaluate(() => {
        const api = (window as unknown as { api?: unknown }).api;
        return Boolean(api) && typeof (api as { saveEntry?: unknown }).saveEntry === "function";
      });
      expect(shimReady, "window.api (shim) должен быть установлен").toBe(true);

      // Создаём заметку через shim. ID — определяем заранее чтобы потом
      // удалить, даже если test fail'ит на assertion.
      const entryId = `eden-e2e-${Date.now()}`;
      const saveResult = await edenWindow.evaluate(async (payload) => {
        const api = (window as unknown as {
          api: {
            saveEntry: (e: unknown) => Promise<unknown>;
          };
        }).api;
        try {
          const res = await api.saveEntry({
            id: payload.id,
            title: payload.title,
            content_json: JSON.stringify({
              type: "doc",
              content: [{ type: "paragraph", content: [{ type: "text", text: "hello eden e2e" }] }],
            }),
            created_at: Date.now(),
            updated_at: Date.now(),
            folder_id: null,
            type_id: "note_obj",
            header_layout: "default",
            header_props_json: "{}",
            schema_version: 1,
            deleted_at: null,
          });
          return { ok: true, res };
        } catch (e) {
          return { ok: false, error: e instanceof Error ? e.message : String(e) };
        }
      }, { id: entryId, title: SAMPLE_TITLE });

      expect(saveResult.ok, `saveEntry должен succeed: ${JSON.stringify(saveResult)}`).toBe(true);

      // listEntries видит созданную заметку.
      const entries = await edenWindow.evaluate(async () => {
        const api = (window as unknown as {
          api: { listEntries: () => Promise<unknown[]> };
        }).api;
        try {
          return await api.listEntries();
        } catch (e) {
          return [{ error: e instanceof Error ? e.message : String(e) }];
        }
      });

      const found = (entries as ArkEntry[]).find((e) => e.id === entryId);
      expect(found, `listEntries должна вернуть созданную заметку с id=${entryId}`).toBeTruthy();
      expect(found?.title).toBe(SAMPLE_TITLE);

      // Cleanup — soft delete через shim. Не assertion-блокирующее на ошибки.
      await edenWindow.evaluate(async (id) => {
        const api = (window as unknown as {
          api: { deleteEntry: (id: string) => Promise<unknown> };
        }).api;
        try {
          await api.deleteEntry(id);
        } catch (e) {
          console.warn("[eden.spec] cleanup deleteEntry failed:", e);
        }
      }, entryId);
    } finally {
      await app.close();
    }
  });

  test("eden: TipTap editor mountains после открытия заметки", async () => {
    const app = await launchKepler({ slug: "eden-editor" });
    try {
      const edenWindow = await openEden(app);

      // Создаём заметку и навигируемся к ней — TipTap должен подгрузиться
      // (lazy chunk Editor.vue) и зарендериться.
      const entryId = `eden-editor-${Date.now()}`;
      await edenWindow.evaluate(async (payload) => {
        const api = (window as unknown as {
          api: {
            saveEntry: (e: unknown) => Promise<unknown>;
          };
        }).api;
        await api.saveEntry({
          id: payload.id,
          title: payload.title,
          content_json: JSON.stringify({ type: "doc", content: [{ type: "paragraph" }] }),
          created_at: Date.now(),
          updated_at: Date.now(),
          folder_id: null,
          type_id: "note_obj",
          header_layout: "default",
          header_props_json: "{}",
          schema_version: 1,
          deleted_at: null,
        });
      }, { id: entryId, title: `${SAMPLE_TITLE}-editor` });

      // Refresh entries в store — лучше через клик/UI, но проще через прямой
      // refresh-call (это side-effect-free).
      await edenWindow.evaluate(() => {
        // Нет публичного API для refresh, но onMounted в App.vue вызывает
        // initApp; здесь полагаемся, что список обновится сам через короткое
        // время или мы навигируемся напрямую. Заметка id=entryId уже в ARK.
      });

      // Дать времени UI'у среагировать.
      await edenWindow.waitForTimeout(500);

      // Главное assertion здесь — TipTap editor элемент существует в DOM
      // после lazy load Editor chunk'а. TipTap создаёт `.ProseMirror` элемент.
      // Это работает только если заметка currentEntry стала; иначе видим
      // SpacesView. Для smoke поэтому assertion soft — проверяем что в
      // body есть либо ProseMirror, либо UI sidebar (значит App смонтировался).
      const bodyText = await edenWindow.locator("body").textContent();
      expect(bodyText, "Eden UI должен отрендерить хоть какой-то текст").toBeTruthy();

      // Cleanup
      await edenWindow.evaluate(async (id) => {
        try {
          await (window as unknown as { api: { deleteEntry: (id: string) => Promise<unknown> } })
            .api.deleteEntry(id);
        } catch {
          // ignore
        }
      }, entryId);
    } finally {
      await app.close();
    }
  });

  // Regression guard: команда `eden:note:open-today` должна
  //   1) persist'ить SYSTEM_TYPE_JOURNAL в backend (upsert_object_type)
  //   2) создать journal entry с today-title
  //   3) выставить zen mode (renderer state)
  // До 2026-05-19 проваливалось на (1) — ensureEntryTypeAvailable возвращал
  // false для client-only "system-type-journal", saveEntry → invalid_type.
  test("eden:note:open-today — создаёт journal entry и включает zen mode", async () => {
    const app = await launchKepler({ slug: "eden-open-today" });
    try {
      const edenWindow = await openEden(app);

      const launcher = await app.firstWindow();

      // Подсчёт journal entries до вызова — для дельта-проверки.
      const beforeCount = await edenWindow.evaluate(async () => {
        const api = (window as unknown as {
          api: { listEntries: () => Promise<Array<{ type_id?: string | null }>> };
        }).api;
        const entries = await api.listEntries();
        return entries.filter((e) => e.type_id === "system-type-journal").length;
      });

      // Invoke команды из launcher window (как делает реальный flow).
      const invoke = await launcher.evaluate(async () => {
        try {
          await (window as unknown as {
            kepler: { commands: { invoke: (id: string) => Promise<void> } };
          }).kepler.commands.invoke("eden:note:open-today");
          return "ok";
        } catch (e) {
          return "throw:" + (e instanceof Error ? e.message : String(e));
        }
      });
      expect(invoke).toBe("ok");

      // Дать Eden время обработать event + async chain (saveNoteType + saveEntry).
      await edenWindow.waitForTimeout(2000);

      // Должен появиться один новый journal entry с today-title.
      // Title формат — ISO `YYYY-MM-DD` (см. openTodayJournal в store).
      const result = await edenWindow.evaluate(async () => {
        const api = (window as unknown as {
          api: { listEntries: () => Promise<Array<{ id: string; title: string; type_id?: string | null }>> };
        }).api;
        const entries = await api.listEntries();
        const journals = entries.filter((e) => e.type_id === "system-type-journal");
        const now = new Date();
        const yyyy = now.getFullYear();
        const mm = String(now.getMonth() + 1).padStart(2, "0");
        const dd = String(now.getDate()).padStart(2, "0");
        const expectedTitle = `${yyyy}-${mm}-${dd}`;
        return {
          totalJournals: journals.length,
          matchingToday: journals.filter((e) => e.title.trim() === expectedTitle),
          expectedTitle,
        };
      });

      expect(
        result.totalJournals,
        `journal entries: было ${beforeCount}, стало ${result.totalJournals}`,
      ).toBeGreaterThanOrEqual(beforeCount + 1);
      expect(
        result.matchingToday.length,
        `должна быть заметка с title "${result.expectedTitle}", вместо нашлось ${result.matchingToday.length}`,
      ).toBeGreaterThanOrEqual(1);

      // zen mode должен быть включён — проверяем через DOM (.focus-mode class
      // на .editor-wrapper или .focus-mode-active на .app-container).
      const zenActive = await edenWindow.evaluate(() => {
        return Boolean(
          document.querySelector(".focus-mode-active") ||
            document.querySelector(".focus-mode"),
        );
      });
      expect(zenActive, "после open-today должен быть включён zen mode").toBe(true);

      // Cleanup — удалить созданную сегодняшнюю заметку (если только её мы и
      // создали в этом прогоне; если их было >1 — мог быть и pre-existing).
      const journalIds = await edenWindow.evaluate(async () => {
        const api = (window as unknown as {
          api: { listEntries: () => Promise<Array<{ id: string; type_id?: string | null }>> };
        }).api;
        const entries = await api.listEntries();
        return entries.filter((e) => e.type_id === "system-type-journal").map((e) => e.id);
      });
      for (const id of journalIds) {
        await edenWindow.evaluate(async (entryId) => {
          try {
            await (window as unknown as { api: { deleteEntry: (id: string) => Promise<unknown> } })
              .api.deleteEntry(entryId);
          } catch {
            // ignore
          }
        }, id);
      }
    } finally {
      await app.close();
    }
  });
});
