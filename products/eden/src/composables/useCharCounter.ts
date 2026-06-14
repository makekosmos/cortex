// useCharCounter — счётчик символов текущей заметки + детектор overlap
// между `.eden-char-counter` overlay'ем и последним блоком ProseMirror.
//
// Logic-block, который раньше жил в App.vue (~80 строк): liveCharCount
// сбрасывается при смене entry, computed currentEntryCharCount предпочитает
// live count либо считает из content_json, ResizeObserver на .ProseMirror
// + window resize/scroll listeners управляют флагом overlap для CSS-border.

import { computed, nextTick, onMounted, onUnmounted, ref, watch } from "vue";
import type { useEdenStore } from "@/store/eden";
import type { useLayoutStore } from "@/store/layout";
import { readEntryMarkdown } from "@/editor-cm/content";

export function useCharCounter(
  eden: ReturnType<typeof useEdenStore>,
  layout: ReturnType<typeof useLayoutStore>,
) {
  const liveCharCount = ref<number | null>(null);

  watch(
    () => eden.currentEntry?.id ?? null,
    () => {
      liveCharCount.value = null;
    },
  );

  const currentEntryCharCount = computed<number | null>(() => {
    if (!eden.currentEntry) return null;
    if (liveCharCount.value !== null) return liveCharCount.value;
    return [...readEntryMarkdown(eden.currentEntry.content_json)].length;
  });

  // Border-top на counter появляется когда last block доходит до counter top.
  // Прямое сравнение bounding rects надёжнее чем overflow detection.
  const charCounterHasOverlap = ref(false);
  let overlapRafId: number | null = null;
  let overlapResizeObserver: ResizeObserver | null = null;

  function recomputeCharCounterOverlap(): void {
    if (!layout.isZenMode) {
      charCounterHasOverlap.value = false;
      return;
    }
    const counterEl = document.querySelector(".eden-char-counter") as HTMLElement | null;
    const proseMirror = document.querySelector(".ProseMirror") as HTMLElement | null;
    if (!counterEl || !proseMirror) {
      charCounterHasOverlap.value = false;
      return;
    }
    const lastBlock = proseMirror.lastElementChild as HTMLElement | null;
    if (!lastBlock) {
      charCounterHasOverlap.value = false;
      return;
    }
    const counterRect = counterEl.getBoundingClientRect();
    const lastBlockRect = lastBlock.getBoundingClientRect();
    // Threshold 4px — bordertop появляется когда text почти доходит до counter.
    charCounterHasOverlap.value = lastBlockRect.bottom > counterRect.top - 4;
  }

  function scheduleOverlapCheck(): void {
    if (overlapRafId !== null) return;
    overlapRafId = requestAnimationFrame(() => {
      overlapRafId = null;
      recomputeCharCounterOverlap();
    });
  }

  // Re-attach observer на ProseMirror когда entry switches (Editor remount'ится
  // при смене заметки, старый PM detached, ResizeObserver на нём перестаёт
  // fire'ить).
  function reattachProseMirrorObserver(): void {
    overlapResizeObserver?.disconnect();
    const pm = document.querySelector(".ProseMirror");
    if (pm) {
      overlapResizeObserver = new ResizeObserver(scheduleOverlapCheck);
      overlapResizeObserver.observe(pm);
    }
    scheduleOverlapCheck();
  }

  watch(liveCharCount, () => scheduleOverlapCheck());
  watch(
    () => layout.isZenMode,
    () => {
      // При переключении zen mode editor relayout'ится — даём DOM settle.
      nextTick(reattachProseMirrorObserver);
    },
  );
  watch(
    () => eden.currentEntry?.id ?? null,
    () => {
      // Switch entry → Editor unmount/remount → PM ref stale.
      nextTick(reattachProseMirrorObserver);
    },
  );

  onMounted(() => {
    window.addEventListener("resize", scheduleOverlapCheck);
    window.addEventListener("scroll", scheduleOverlapCheck, { passive: true, capture: true });
    nextTick(reattachProseMirrorObserver);
  });

  onUnmounted(() => {
    window.removeEventListener("resize", scheduleOverlapCheck);
    window.removeEventListener("scroll", scheduleOverlapCheck, {
      capture: true,
    } as EventListenerOptions);
    overlapResizeObserver?.disconnect();
    overlapResizeObserver = null;
    if (overlapRafId !== null) cancelAnimationFrame(overlapRafId);
  });

  function pluralizeCharacters(n: number): string {
    const mod10 = n % 10;
    const mod100 = n % 100;
    if (mod10 === 1 && mod100 !== 11) return "символ";
    if (mod10 >= 2 && mod10 <= 4 && (mod100 < 10 || mod100 >= 20)) return "символа";
    return "символов";
  }

  return {
    liveCharCount,
    currentEntryCharCount,
    charCounterHasOverlap,
    pluralizeCharacters,
  };
}
