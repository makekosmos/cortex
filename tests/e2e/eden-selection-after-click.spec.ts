// Eden block-selection after click — regression test для bug:
//   - Rubber-band drag по двум task'ам → они выделены (.kepler-block-selected).
//   - Клик на одну из task'ей.
//   - Bug: визуально (через DOM classes / background-color) задачи всё ещё
//     выглядят выделенными.
//
// Этот spec проверяет МНОЖЕСТВО scenario'ев клика чтобы найти конкретный
// сценарий где баг ещё воспроизводится. Каждый scenario — отдельный
// test(...). Общая логика setup'а вынесена в helper'ы.

import { test, expect, type Page, type ElectronApplication } from "@playwright/test";
import { _electron } from "playwright";
import { launchKepler } from "./helpers/launch";

type DiagSnapshot = {
  blockSelected: number;
  rangeSelected: number;
  nodeSelected: number;
  tasks: Array<{ idx: number; cls: string; bg: string; rowBg: string | null; hasBlockSel: boolean }>;
  sel: { from: number; to: number; type: string; empty: boolean } | null;
};

async function snapshot(page: Page): Promise<DiagSnapshot> {
  return await page.evaluate(() => {
    const blockSelected = document.querySelectorAll(".kepler-block-selected").length;
    const rangeSelected = document.querySelectorAll(".task-ref-node.is-range-selected").length;
    const nodeSelected = document.querySelectorAll(".task-ref-node.is-node-selected").length;
    const taskNodes = Array.from(document.querySelectorAll(".task-ref-node")) as HTMLElement[];
    const tasks = taskNodes.map((el, idx) => {
      const cls = el.className;
      const bg = getComputedStyle(el).backgroundColor;
      const row = el.querySelector(".task-ref-row") as HTMLElement | null;
      const rowBg = row ? getComputedStyle(row).backgroundColor : null;
      const hasBlockSel =
        el.classList.contains("kepler-block-selected") ||
        !!el.querySelector(".kepler-block-selected");
      return { idx, cls, bg, rowBg, hasBlockSel };
    });
    const view = (window as any).__edenEditor?.view ?? (window as any).__edenView;
    const sel = view?.state?.selection
      ? {
          from: view.state.selection.from,
          to: view.state.selection.to,
          type: view.state.selection.constructor?.name ?? "",
          empty: !!view.state.selection.empty,
        }
      : null;
    return { blockSelected, rangeSelected, nodeSelected, tasks, sel };
  });
}

function countVisuallySelected(snap: DiagSnapshot): number {
  // «Выделенной» считаем задачу с реально видимым фоном. Классы
  // `is-node-selected` / `is-range-selected` / `.ProseMirror-selectednode`
  // остаются на DOM (PM семантика), но CSS на них больше не реагирует —
  // меряем фактический backgroundColor через getComputedStyle.
  return snap.tasks.filter((t) => {
    if (t.hasBlockSel) return true;
    const bg = t.rowBg ?? t.bg;
    if (!bg) return false;
    // rgba(0,0,0,0) или transparent → нет фона
    if (bg === "rgba(0, 0, 0, 0)" || bg === "transparent") return false;
    return true;
  }).length;
}

async function setupEdenWithTasks(taskCount: number): Promise<{
  app: ElectronApplication;
  edenWin: Page;
}> {
  const app = await launchKepler({ slug: `eden-sel-${Date.now()}-${Math.floor(Math.random() * 1000)}` });
  const launcher = await app.firstWindow();
  await launcher.waitForLoadState("domcontentloaded");
  await launcher.waitForTimeout(2500);

  await app.evaluate(async ({ BrowserWindow }) => {
    const win = BrowserWindow.getAllWindows()[0];
    await win!.webContents.executeJavaScript(`window.kepler.commands.invoke("eden:open")`);
  });

  const edenWin = await app.waitForEvent("window", { timeout: 10_000 });
  await edenWin.waitForLoadState("domcontentloaded");
  edenWin.on("console", (msg) => {
    const text = msg.text();
    if (text.includes("eden") || text.includes("selection") || text.includes("[diag]")) {
      console.log("[browser]", text);
    }
  });
  await edenWin.waitForTimeout(2000);

  const noteId = `sel-click-${Date.now()}-${Math.floor(Math.random() * 1000)}`;
  await edenWin.evaluate(async (id) => {
    const result = await window.api.saveEntry({
      id,
      title: "Selection click test",
      content_json: JSON.stringify({ type: "doc", content: [{ type: "paragraph" }] }),
      created_at: Date.now(),
      updated_at: Date.now(),
      folder_id: null,
      type_id: "note_obj",
      header_layout: "default",
      header_props_json: "{}",
      schema_version: 1,
    } as any);
    if (!result?.ok) throw new Error(`saveEntry failed: ${JSON.stringify(result)}`);
  }, noteId);

  await edenWin
    .evaluate(async (id) => {
      const mod = await import("/src/store/eden.ts");
      const store = mod.useEdenStore();
      await store.refreshEntries();
      await store.navigateTo(id);
    }, noteId)
    .catch(() => {});
  await edenWin.waitForTimeout(1000);

  // Создаём task'и через /задача → Enter → текст → Enter (на новой строке снова /задача)
  const pm = edenWin.locator(".ProseMirror").first();
  await pm.click();

  for (let i = 0; i < taskCount; i++) {
    if (i > 0) {
      // Blur input предыдущей task'и → курсор в новый paragraph внутри PM
      const lastInput = edenWin.locator(".task-ref-title-input").nth(i - 1);
      await lastInput.press("Enter").catch(() => {});
      await edenWin.waitForTimeout(200);
      // Кликаем в editor чтобы фокус был в PM, а не в input
      await pm.click({ position: { x: 200, y: 400 } }).catch(() => {});
      await edenWin.waitForTimeout(200);
    }
    await edenWin.keyboard.type("/задача");
    await edenWin.waitForTimeout(400);
    await edenWin.keyboard.press("Enter");
    await edenWin.waitForTimeout(700);

    const input = edenWin.locator(".task-ref-title-input").nth(i);
    await input.click();
    await input.fill(`task ${i + 1}`);
    await edenWin.waitForTimeout(200);
  }

  const got = await edenWin.locator(".task-ref-node").count();
  console.log(`[test] created ${got}/${taskCount} tasks`);
  expect(got).toBeGreaterThanOrEqual(taskCount);

  // Снимаем фокус с input'а
  await edenWin.locator(".editor-header").first().click({ force: true }).catch(() => {});
  await edenWin.waitForTimeout(300);

  return { app, edenWin };
}

async function rubberBandSelectFirstTwo(edenWin: Page): Promise<void> {
  const geom = await edenWin.evaluate(() => {
    const taskNodes = Array.from(document.querySelectorAll(".task-ref-node")) as HTMLElement[];
    const contentArea = document.querySelector(".editor-content-area") as HTMLElement;
    const pm = document.querySelector(".ProseMirror") as HTMLElement;
    const rects = taskNodes.map((el) => {
      const r = el.getBoundingClientRect();
      return { left: r.left, top: r.top, right: r.right, bottom: r.bottom };
    });
    const car = contentArea.getBoundingClientRect();
    const pmr = pm.getBoundingClientRect();
    return {
      rects,
      contentArea: { left: car.left, top: car.top, right: car.right, bottom: car.bottom },
      pm: { left: pmr.left, top: pmr.top, right: pmr.right, bottom: pmr.bottom },
    };
  });
  if (geom.rects.length < 2) throw new Error(`need 2 task rects, got ${geom.rects.length}`);

  const startX = Math.max(geom.contentArea.left + 5, geom.pm.left - 30);
  const startY = geom.rects[0].top + 2;
  const endX = geom.pm.right - 5;
  const endY = geom.rects[1].bottom - 2;

  await edenWin.mouse.move(startX, startY);
  await edenWin.mouse.down({ button: "left" });
  await edenWin.mouse.move(startX + 5, startY + 5, { steps: 3 });
  await edenWin.mouse.move((startX + endX) / 2, (startY + endY) / 2, { steps: 5 });
  await edenWin.mouse.move(endX, endY, { steps: 5 });
  await edenWin.waitForTimeout(200);
  await edenWin.mouse.up({ button: "left" });
  await edenWin.waitForTimeout(500);
}

async function assertRubberBandSucceeded(edenWin: Page, scenario: string): Promise<void> {
  const snap = await snapshot(edenWin);
  console.log(`[diag][${scenario}] AFTER DRAG:`, JSON.stringify(snap, null, 2));
  if (snap.blockSelected < 2) {
    console.log(`[test][${scenario}] WARN: rubber-band не выделил 2 task'и (blockSelected=${snap.blockSelected})`);
  }
}

function assertClean(snap: DiagSnapshot, scenario: string): void {
  console.log(`[diag][${scenario}] AFTER CLICK:`, JSON.stringify(snap, null, 2));
  const visuallySelected = countVisuallySelected(snap);
  console.log(`[test][${scenario}] visually-selected count =`, visuallySelected);

  // После обычного клика на задачу — НИКАКОГО visual highlight'а быть
  // не должно. Юзер «просто кликнул» — нет основания подсвечивать блок.
  // PM-NodeSelection семантически жива (для Delete-key), но CSS на это
  // не реагирует (см. TaskRefView.vue scoped styles).
  expect(
    snap.blockSelected,
    `[${scenario}] .kepler-block-selected должно быть 0, got ${snap.blockSelected}`,
  ).toBe(0);
  expect(
    visuallySelected,
    `[${scenario}] после простого клика никакого визуального выделения быть не должно, got ${visuallySelected}; tasks=${JSON.stringify(snap.tasks)}`,
  ).toBe(0);
}

test.describe("Eden block-selection after click — scenarios", () => {
  test.setTimeout(120_000);

  test("A. click на title INPUT первой task'и", async () => {
    const { app, edenWin } = await setupEdenWithTasks(2);
    try {
      await rubberBandSelectFirstTwo(edenWin);
      await assertRubberBandSucceeded(edenWin, "A");

      await edenWin.bringToFront();
      const input = edenWin.locator(".task-ref-title-input").first();
      // Заполним input реальным текстом чтобы было КУДА ставить каретку
      // (на пустом input click-position не отличается от click-anywhere).
      await input.fill("hello world task title");
      await edenWin.waitForTimeout(100);

      // Snapshot ДО click — что в фокусе после fill?
      const focusAfterFill = await edenWin.evaluate(() => ({
        tag: document.activeElement?.tagName,
        cls: document.activeElement?.className,
        hasWindowFocus: document.hasFocus(),
      }));
      console.log(`[focus][A] after fill:`, JSON.stringify(focusAfterFill));

      const box = await input.boundingBox();
      if (!box) throw new Error("input has no bbox");
      // Кликаем НЕ в центр, а в правую треть — там должна стоять каретка
      // если фокус на input (около середины слова).
      const clickX = box.x + box.width * 0.6;
      const clickY = box.y + box.height / 2;
      await edenWin.mouse.click(clickX, clickY);
      await edenWin.waitForTimeout(500);

      // Снимок: что в фокусе, какая PM selection, где курсор
      const focus = await edenWin.evaluate(() => {
        const el = document.activeElement as HTMLElement | null;
        const editor = (window as any).__edenEditor;
        const sel = editor?.state?.selection;
        return {
          activeTag: el?.tagName ?? null,
          activeCls: el?.className ?? null,
          inputCaret: el?.tagName === "INPUT" ? (el as HTMLInputElement).selectionStart : null,
          inputValue: el?.tagName === "INPUT" ? (el as HTMLInputElement).value : null,
          pmFrom: sel?.from ?? null,
          pmTo: sel?.to ?? null,
        };
      });
      console.log(`[focus][A]`, JSON.stringify(focus));

      const snap = await snapshot(edenWin);
      assertClean(snap, "A");

      // Главный assert: после клика на task title input — фокус ДОЛЖЕН
      // быть на этом input, не на <p> и не где-либо ещё.
      expect(focus.activeTag, "после клика на title — focus должен быть на INPUT").toBe("INPUT");
      expect(focus.activeCls, "класс активного элемента должен быть task-ref-title-input").toContain("task-ref-title-input");
      // Каретка не должна торчать в 0 (т.е. в начале) если клик был
      // в середине input'а с текстом длиннее 5 символов.
      expect(focus.inputCaret, "каретка должна стоять примерно где кликнули, не в 0").toBeGreaterThan(2);
    } finally {
      await app.close();
    }
  });

  test("B. click на ТРЕТЬЮ task (не в rubber-band)", async () => {
    const { app, edenWin } = await setupEdenWithTasks(3);
    try {
      await rubberBandSelectFirstTwo(edenWin);
      await assertRubberBandSucceeded(edenWin, "B");

      const third = edenWin.locator(".task-ref-node").nth(2);
      const box = await third.boundingBox();
      if (!box) throw new Error("third task has no bbox");
      await edenWin.mouse.click(box.x + box.width / 2, box.y + box.height / 2);
      await edenWin.waitForTimeout(500);

      const snap = await snapshot(edenWin);
      assertClean(snap, "B");
    } finally {
      await app.close();
    }
  });

  test("C. click на task-row (контейнер, не input)", async () => {
    const { app, edenWin } = await setupEdenWithTasks(2);
    try {
      await rubberBandSelectFirstTwo(edenWin);
      await assertRubberBandSucceeded(edenWin, "C");

      // Клик правее input'а — внутри row но в padding area.
      const row = edenWin.locator(".task-ref-row").first();
      const box = await row.boundingBox();
      if (!box) throw new Error("row has no bbox");
      await edenWin.mouse.click(box.x + box.width - 8, box.y + box.height / 2);
      await edenWin.waitForTimeout(500);

      const snap = await snapshot(edenWin);
      assertClean(snap, "C");
    } finally {
      await app.close();
    }
  });

  test("D. click с минимальным mouse jitter (1-2px)", async () => {
    const { app, edenWin } = await setupEdenWithTasks(2);
    try {
      await rubberBandSelectFirstTwo(edenWin);
      await assertRubberBandSucceeded(edenWin, "D");

      const target = edenWin.locator(".task-ref-node").first();
      const box = await target.boundingBox();
      if (!box) throw new Error("first task has no bbox");
      const cx = box.x + box.width / 2;
      const cy = box.y + box.height / 2;
      await edenWin.mouse.move(cx, cy);
      await edenWin.mouse.down({ button: "left" });
      await edenWin.mouse.move(cx + 2, cy + 1);
      await edenWin.mouse.up({ button: "left" });
      await edenWin.waitForTimeout(500);

      const snap = await snapshot(edenWin);
      assertClean(snap, "D");
    } finally {
      await app.close();
    }
  });

  test("E. click ПОСЛЕ Esc — двухступенчатый clear", async () => {
    const { app, edenWin } = await setupEdenWithTasks(2);
    try {
      await rubberBandSelectFirstTwo(edenWin);
      await assertRubberBandSucceeded(edenWin, "E");

      await edenWin.keyboard.press("Escape");
      await edenWin.waitForTimeout(300);

      const afterEsc = await snapshot(edenWin);
      console.log("[diag][E] AFTER ESC:", JSON.stringify(afterEsc, null, 2));

      const target = edenWin.locator(".task-ref-node").first();
      const box = await target.boundingBox();
      if (!box) throw new Error("first task has no bbox");
      await edenWin.mouse.click(box.x + box.width / 2, box.y + box.height / 2);
      await edenWin.waitForTimeout(500);

      const snap = await snapshot(edenWin);
      assertClean(snap, "E");
    } finally {
      await app.close();
    }
  });

  test("F. right-click (ПКМ) на task — открыть context menu", async () => {
    const { app, edenWin } = await setupEdenWithTasks(2);
    try {
      await rubberBandSelectFirstTwo(edenWin);
      await assertRubberBandSucceeded(edenWin, "F");

      const target = edenWin.locator(".task-ref-row").first();
      const box = await target.boundingBox();
      if (!box) throw new Error("row has no bbox");
      await edenWin.mouse.click(box.x + box.width / 2, box.y + box.height / 2, { button: "right" });
      await edenWin.waitForTimeout(500);

      const snap = await snapshot(edenWin);
      assertClean(snap, "F");
    } finally {
      await app.close();
    }
  });

  test("G. click на чекбокс (TaskStatusIcon) первой task'и", async () => {
    const { app, edenWin } = await setupEdenWithTasks(2);
    try {
      await rubberBandSelectFirstTwo(edenWin);
      await assertRubberBandSucceeded(edenWin, "G");

      const icon = edenWin.locator(".task-ref-status").first();
      const box = await icon.boundingBox();
      if (!box) throw new Error("status icon has no bbox");
      await edenWin.mouse.click(box.x + box.width / 2, box.y + box.height / 2);
      await edenWin.waitForTimeout(500);

      const snap = await snapshot(edenWin);
      assertClean(snap, "G");
    } finally {
      await app.close();
    }
  });
});
