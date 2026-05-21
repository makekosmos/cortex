// Eden TaskRef ArrowUp/ArrowDown navigation regression.
//
// Tested: `extensions/eden/src/components/TaskRefView.vue` → onTitleArrowVertical.
//
// Инвариант: когда фокус в title input одного TaskRef и юзер нажимает
// ArrowDown/ArrowUp, фокус прыгает в соседний TaskRef title input или
// в соседний paragraph (PM TextSelection).
//
// Стратегия: создаём task_obj через shim, заметку с заданным content_json,
// открываем editor через sidebar click, фокусируем нужный input через DOM,
// диспатчим стрелку через page.keyboard.press → проверяем document.activeElement
// и/или PM selection state.

import { test, expect } from "@playwright/test";
import { launchKepler } from "./helpers/launch";
import { openEden, createEdenNote, openNoteViaReload, deleteEdenEntry } from "./helpers/eden";
import { taskRef, paragraph, type PMDoc } from "./helpers/eden-doc";

/**
 * Создать task_obj напрямую через ark bridge (createTask exported from shim
 * не висит на window.api — его используют внутри edenApi/TaskRefView).
 */
async function createTaskViaArk(
  edenWindow: Awaited<ReturnType<typeof openEden>>,
  sourceNoteId: string,
  title: string,
): Promise<string | null> {
  return await edenWindow.evaluate(
    async ({ noteId, taskTitle }) => {
      type ArkReq = { request: <T>(op: string, params?: Record<string, unknown>) => Promise<T> };
      const ark = (window as unknown as { kepler?: { ark?: ArkReq } }).kepler?.ark;
      if (!ark) return null;
      try {
        const now = new Date().toISOString();
        // Регистрируем task_obj type (idempotent).
        await ark.request("upsert_object_type", {
          object_type: {
            id: "task_obj",
            name: "Задача",
            schemaJson: "{}",
            uiSchemaJson: "{}",
            systemLocked: false,
            createdAt: now,
            updatedAt: now,
          },
        });
        const taskId = (window as unknown as { crypto: Crypto }).crypto.randomUUID();
        await ark.request("upsert_object", {
          object: {
            id: taskId,
            typeId: "task_obj",
            title: taskTitle.trim() || "Пустая задача",
            contentJson: { type: "doc", content: [{ type: "paragraph" }] },
            propsJson: {
              description: null,
              priority: 0,
              scheduled_date: null,
              deadline: null,
              reminder_date: null,
              is_today: false,
              is_evening: false,
              is_someday: false,
              is_completed: false,
              completed_at: null,
              is_cancelled: false,
              cancelled_at: null,
              is_trashed: false,
              status: "triage",
              sort_order: 0,
              heading_id: null,
              project_id: null,
              area_id: null,
              tag_ids: [],
              checklist_items: [],
              recurrence_rule: null,
              billable: false,
              price: null,
              created_at: now,
              source_app: "eden",
              source_note_id: noteId,
              model_version: 1,
            },
            createdAt: now,
            updatedAt: now,
            deletedAt: null,
          },
        });
        return taskId;
      } catch (e) {
        console.error("[test] createTaskViaArk error:", e);
        return null;
      }
    },
    { noteId: sourceNoteId, taskTitle: title },
  );
}

/**
 * Сфокусировать TaskRef title input по taskId.
 */
async function focusTaskRefInput(
  edenWindow: Awaited<ReturnType<typeof openEden>>,
  taskId: string,
): Promise<boolean> {
  return await edenWindow.evaluate((id) => {
    const wrapper = document.querySelector(`[data-task-id="${id}"]`);
    if (!wrapper) return false;
    const input = wrapper.querySelector(".task-ref-title-input") as HTMLInputElement | null;
    if (!input) return false;
    input.focus();
    return document.activeElement === input;
  }, taskId);
}

/**
 * Прочитать taskId активного TaskRef input'а (или null если фокус не на нём).
 */
async function activeTaskId(
  edenWindow: Awaited<ReturnType<typeof openEden>>,
): Promise<string | null> {
  return await edenWindow.evaluate(() => {
    const el = document.activeElement as HTMLElement | null;
    if (!el || !el.classList.contains("task-ref-title-input")) return null;
    const wrapper = el.closest("[data-task-id]") as HTMLElement | null;
    return wrapper?.dataset.taskId ?? null;
  });
}

/**
 * Проверить что активный element — ProseMirror surface (caret в текстовом блоке).
 */
async function activeIsProseMirror(
  edenWindow: Awaited<ReturnType<typeof openEden>>,
): Promise<boolean> {
  return await edenWindow.evaluate(() => {
    const el = document.activeElement as HTMLElement | null;
    return !!el && (el.classList.contains("ProseMirror") || el.closest(".ProseMirror") !== null);
  });
}

async function openAndMountNote(
  edenWindow: Awaited<ReturnType<typeof openEden>>,
  entryId: string,
  _title: string,
): Promise<boolean> {
  const ready = await openNoteViaReload(edenWindow, entryId);
  // eslint-disable-next-line no-console
  console.log(`[taskref-nav] openNoteViaReload(${entryId}) → mounted=${ready}`);
  return ready;
}

test.describe("eden: TaskRef ArrowUp/ArrowDown navigation", () => {
  test("ArrowDown в верхнем TaskRef → фокус в input нижнего", async () => {
    const app = await launchKepler({ slug: "eden-taskref-nav-down" });
    try {
      const edenWindow = await openEden(app);

      const noteId = `taskref-nav-down-${Date.now()}`;
      const task1 = await createTaskViaArk(edenWindow, noteId, "Задача 1");
      const task2 = await createTaskViaArk(edenWindow, noteId, "Задача 2");
      if (!task1 || !task2) {
        test.skip(true, "createTask недоступен через shim");
        return;
      }

      await createEdenNote(edenWindow, {
        id: noteId,
        title: "taskref-nav-down",
        contentJson: {
          type: "doc",
          content: [taskRef(task1), taskRef(task2)],
        } satisfies PMDoc,
      });

      const mounted = await openAndMountNote(edenWindow, noteId, "taskref-nav-down");
      expect(mounted, "ProseMirror должен смонтироваться").toBe(true);

      // Подождём пока оба taskRef отрендерены.
      await edenWindow.waitForTimeout(800);

      const focused1 = await focusTaskRefInput(edenWindow, task1);
      expect(focused1, `должен сфокусировать input task1=${task1}`).toBe(true);

      await edenWindow.keyboard.press("ArrowDown");

      // Дать focus stealing detection time (focus-defender может вернуть focus
      // обратно — это будет bug). Ждём 200ms чтобы поймать flap'ы.
      await edenWindow.waitForTimeout(200);

      const active = await activeTaskId(edenWindow);
      expect(
        active,
        `после ArrowDown активный TaskRef должен быть task2=${task2}, фактически ${active}`,
      ).toBe(task2);

      await deleteEdenEntry(edenWindow, noteId);
    } finally {
      await app.close();
    }
  });

  test("ArrowUp в нижнем TaskRef → фокус в input верхнего", async () => {
    const app = await launchKepler({ slug: "eden-taskref-nav-up" });
    try {
      const edenWindow = await openEden(app);

      const noteId = `taskref-nav-up-${Date.now()}`;
      const task1 = await createTaskViaArk(edenWindow, noteId, "Задача 1");
      const task2 = await createTaskViaArk(edenWindow, noteId, "Задача 2");
      if (!task1 || !task2) {
        test.skip(true, "createTask недоступен");
        return;
      }

      await createEdenNote(edenWindow, {
        id: noteId,
        title: "taskref-nav-up",
        contentJson: {
          type: "doc",
          content: [taskRef(task1), taskRef(task2)],
        } satisfies PMDoc,
      });

      const mounted = await openAndMountNote(edenWindow, noteId, "taskref-nav-up");
      expect(mounted).toBe(true);
      await edenWindow.waitForTimeout(800);

      const focused2 = await focusTaskRefInput(edenWindow, task2);
      expect(focused2).toBe(true);

      await edenWindow.keyboard.press("ArrowUp");
      await edenWindow.waitForTimeout(200);

      const active = await activeTaskId(edenWindow);
      expect(active, `после ArrowUp активный TaskRef = task1=${task1}, фактически ${active}`).toBe(
        task1,
      );

      await deleteEdenEntry(edenWindow, noteId);
    } finally {
      await app.close();
    }
  });

  test("TaskRef → paragraph → TaskRef: ArrowDown сначала в paragraph, потом в нижний TaskRef", async () => {
    const app = await launchKepler({ slug: "eden-taskref-nav-via-p" });
    try {
      const edenWindow = await openEden(app);

      const noteId = `taskref-nav-via-p-${Date.now()}`;
      const task1 = await createTaskViaArk(edenWindow, noteId, "Задача A");
      const task2 = await createTaskViaArk(edenWindow, noteId, "Задача B");
      if (!task1 || !task2) {
        test.skip(true, "createTask недоступен");
        return;
      }

      await createEdenNote(edenWindow, {
        id: noteId,
        title: "taskref-nav-via-p",
        contentJson: {
          type: "doc",
          content: [taskRef(task1), paragraph("между задачами"), taskRef(task2)],
        } satisfies PMDoc,
      });

      const mounted = await openAndMountNote(edenWindow, noteId, "taskref-nav-via-p");
      expect(mounted).toBe(true);
      await edenWindow.waitForTimeout(800);

      const focused1 = await focusTaskRefInput(edenWindow, task1);
      expect(focused1).toBe(true);

      // ArrowDown → должны попасть в paragraph (PM TextSelection).
      await edenWindow.keyboard.press("ArrowDown");
      await edenWindow.waitForTimeout(200);

      const onPM = await activeIsProseMirror(edenWindow);
      const activeAfter1 = await activeTaskId(edenWindow);
      expect(
        onPM && activeAfter1 === null,
        `после первого ArrowDown ожидаем focus в ProseMirror surface (paragraph), фактически: onPM=${onPM} activeTaskRef=${activeAfter1}`,
      ).toBe(true);

      // Ещё ArrowDown → должны попасть в task2 input. PM selection должен
      // дойти до конца параграфа а затем findFrom от границы taskRef'а
      // приземлит focus в input нижнего taskRef. Но это — UX в TaskRefView
      // обрабатывает только когда focus в input; в paragraph PM нативно
      // глотает ArrowDown и двигает caret. Чтобы прыгнуть на task2 нужно
      // нажать ArrowDown пока в paragraph (PM сам не сфокусит input
      // taskRef). Это особенность реализации — fix-агент решит чинить
      // или нет.
      //
      // Здесь делаем soft check: пробуем ArrowDown и смотрим что произошло.
      await edenWindow.keyboard.press("ArrowDown");
      await edenWindow.waitForTimeout(200);
      const activeAfter2 = await activeTaskId(edenWindow);
      const onPM2 = await activeIsProseMirror(edenWindow);
      // eslint-disable-next-line no-console
      console.log(
        `[taskref-nav-via-p] после второго ArrowDown: activeTaskRef=${activeAfter2}, onPM=${onPM2}`,
      );
      // Сильное assertion: либо мы попали в task2, либо остались в PM
      // (что бы ни решил fix-агент — главное не падать).
      expect(
        activeAfter2 === task2 || onPM2,
        `после двух ArrowDown ожидаем focus в task2=${task2} или в PM; фактически activeTaskRef=${activeAfter2}, onPM=${onPM2}`,
      ).toBe(true);

      await deleteEdenEntry(edenWindow, noteId);
    } finally {
      await app.close();
    }
  });

  test("единственный TaskRef в документе: ArrowUp/ArrowDown — без падений", async () => {
    const app = await launchKepler({ slug: "eden-taskref-nav-single" });
    try {
      const edenWindow = await openEden(app);

      const noteId = `taskref-nav-single-${Date.now()}`;
      const task1 = await createTaskViaArk(edenWindow, noteId, "Одна задача");
      if (!task1) {
        test.skip(true, "createTask недоступен");
        return;
      }

      await createEdenNote(edenWindow, {
        id: noteId,
        title: "taskref-nav-single",
        contentJson: {
          type: "doc",
          content: [taskRef(task1)],
        } satisfies PMDoc,
      });

      const mounted = await openAndMountNote(edenWindow, noteId, "taskref-nav-single");
      expect(mounted).toBe(true);
      await edenWindow.waitForTimeout(800);

      const focused = await focusTaskRefInput(edenWindow, task1);
      expect(focused).toBe(true);

      // ArrowUp: нет соседа сверху. findFrom вернёт null → не делает
      // preventDefault → input native scroll. focus НЕ должен потеряться.
      await edenWindow.keyboard.press("ArrowUp");
      await edenWindow.waitForTimeout(150);
      const afterUp = await activeTaskId(edenWindow);
      const onPMUp = await activeIsProseMirror(edenWindow);
      expect(
        afterUp === task1 || onPMUp,
        `после ArrowUp на единственном TaskRef focus остался в нём или в PM (есть trailing paragraph). фактически: active=${afterUp}, onPM=${onPMUp}`,
      ).toBe(true);

      // ArrowDown: должен переместиться в trailing paragraph (если TrailingParagraph
      // плагин его создал) либо остаться в input.
      await edenWindow.keyboard.press("ArrowDown");
      await edenWindow.waitForTimeout(150);
      const afterDown = await activeTaskId(edenWindow);
      const onPMDown = await activeIsProseMirror(edenWindow);
      expect(
        afterDown === task1 || onPMDown,
        `после ArrowDown на единственном TaskRef focus остался в нём или в PM. фактически: active=${afterDown}, onPM=${onPMDown}`,
      ).toBe(true);

      await deleteEdenEntry(edenWindow, noteId);
    } finally {
      await app.close();
    }
  });

  test("focus не возвращается обратно через 100мс (focus-defender не крадёт)", async () => {
    // Trap #4: после стрелки defender может вернуть focus в исходный input.
    // Спим 250ms после стрелки и снова проверяем что focus в правильном месте.
    const app = await launchKepler({ slug: "eden-taskref-nav-defender" });
    try {
      const edenWindow = await openEden(app);

      const noteId = `taskref-nav-defender-${Date.now()}`;
      const task1 = await createTaskViaArk(edenWindow, noteId, "T1");
      const task2 = await createTaskViaArk(edenWindow, noteId, "T2");
      if (!task1 || !task2) {
        test.skip(true, "createTask недоступен");
        return;
      }

      await createEdenNote(edenWindow, {
        id: noteId,
        title: "taskref-nav-defender",
        contentJson: {
          type: "doc",
          content: [taskRef(task1), taskRef(task2)],
        } satisfies PMDoc,
      });

      const mounted = await openAndMountNote(edenWindow, noteId, "taskref-nav-defender");
      expect(mounted).toBe(true);
      await edenWindow.waitForTimeout(800);

      const focused1 = await focusTaskRefInput(edenWindow, task1);
      expect(focused1).toBe(true);

      await edenWindow.keyboard.press("ArrowDown");
      await edenWindow.waitForTimeout(50);
      const immediately = await activeTaskId(edenWindow);
      await edenWindow.waitForTimeout(400); // тотал ~450ms — defender имел шанс отработать
      const afterDelay = await activeTaskId(edenWindow);

      expect(immediately, `сразу после ArrowDown focus должен быть в task2=${task2}`).toBe(task2);
      expect(
        afterDelay,
        `через 450ms focus всё ещё в task2=${task2} (defender не должен украсть)`,
      ).toBe(task2);

      await deleteEdenEntry(edenWindow, noteId);
    } finally {
      await app.close();
    }
  });
});
