import { chromium } from "playwright";
import { pathToFileURL } from "node:url";
import path from "node:path";

const htmlPath = path.resolve(
  ".agent/tasks/2026-05-28-eden-scroll-drag-select/smoke/eden-scroll-drag-select.html",
);

const browser = await chromium.launch({ headless: true });
const page = await browser.newPage({ viewport: { width: 900, height: 600 } });

try {
  await page.goto(pathToFileURL(htmlPath).href);
  const result = await page.evaluate(() => {
    const wrapper = document.querySelector(".editor-wrapper");
    const content = document.querySelector(".editor-content-area");
    const paragraph = document.querySelector(".ProseMirror p");
    if (!(wrapper instanceof HTMLElement)) throw new Error("missing editor-wrapper");
    if (!(content instanceof HTMLElement)) throw new Error("missing editor-content-area");
    if (!(paragraph instanceof HTMLElement)) throw new Error("missing paragraph");

    const initial = getComputedStyle(wrapper);
    const scrollPaddingTop = initial.scrollPaddingTop;
    const scrollPaddingBottom = initial.scrollPaddingBottom;
    const initialOverflowY = initial.overflowY;

    content.classList.add("kepler-block-select-active");
    const persistedSelectionOverflowY = getComputedStyle(wrapper).overflowY;
    const persistedSelectionPointerEvents = getComputedStyle(paragraph).pointerEvents;

    content.classList.add("kepler-block-drag-active");
    const dragOverflowY = getComputedStyle(wrapper).overflowY;

    content.classList.remove("kepler-block-drag-active");
    const afterDragOverflowY = getComputedStyle(wrapper).overflowY;

    return {
      scrollPaddingTop,
      scrollPaddingBottom,
      initialOverflowY,
      persistedSelectionOverflowY,
      persistedSelectionPointerEvents,
      dragOverflowY,
      afterDragOverflowY,
    };
  });

  const failures = [];
  if (result.scrollPaddingTop !== "auto" && result.scrollPaddingTop !== "0px") {
    failures.push(`scrollPaddingTop=${result.scrollPaddingTop}`);
  }
  if (result.scrollPaddingBottom !== "auto" && result.scrollPaddingBottom !== "0px") {
    failures.push(`scrollPaddingBottom=${result.scrollPaddingBottom}`);
  }
  if (result.initialOverflowY !== "auto") {
    failures.push(`initialOverflowY=${result.initialOverflowY}`);
  }
  if (result.persistedSelectionOverflowY !== "auto") {
    failures.push(`persistedSelectionOverflowY=${result.persistedSelectionOverflowY}`);
  }
  if (result.persistedSelectionPointerEvents === "none") {
    failures.push("persisted selection disables pointer events");
  }
  if (result.dragOverflowY !== "hidden") {
    failures.push(`dragOverflowY=${result.dragOverflowY}`);
  }
  if (result.afterDragOverflowY !== "auto") {
    failures.push(`afterDragOverflowY=${result.afterDragOverflowY}`);
  }

  if (failures.length > 0) {
    console.error(JSON.stringify({ result, failures }, null, 2));
    process.exitCode = 1;
  } else {
    console.log(JSON.stringify({ result, verdict: "PASS" }, null, 2));
  }
} finally {
  await browser.close();
}
