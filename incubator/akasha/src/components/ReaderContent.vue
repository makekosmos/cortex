<script setup lang="ts">
import { BookOpen } from "@lucide/vue";
import { computed, useTemplateRef, watch } from "vue";
import type { EpubBook, ReaderBlock } from "../lib/epub";

const props = defineProps<{
  book: EpubBook | null;
  activeChapterId: string | null;
  activeBlockId: string | null;
  fontFamily: string;
  fontSize: number;
}>();

const emit = defineEmits<{
  activeChapterChange: [chapterId: string];
  activeLocationChange: [location: { chapterId: string; blockId: string; percentage: number }];
}>();

const scroller = useTemplateRef<HTMLElement>("scroller");
const contentStyle = computed(() => ({
  fontFamily: props.fontFamily,
  fontSize: `${props.fontSize}px`,
}));

watch(
  () => props.activeBlockId,
  (blockId) => {
    if (!blockId) return;
    const target = scroller.value?.querySelector<HTMLElement>(`[data-block-id="${blockId}"]`);
    target?.scrollIntoView({ block: "start" });
  },
);

function onScroll() {
  const root = scroller.value;
  const currentBook = props.book;
  if (!root || !currentBook) return;
  const marker = root.getBoundingClientRect().top + 72;
  let current: HTMLElement | null = null;

  for (const block of Array.from(root.querySelectorAll<HTMLElement>("[data-block-id]"))) {
    if (block.getBoundingClientRect().top <= marker) {
      current = block;
    } else {
      break;
    }
  }

  const chapterId = current?.dataset.chapterId;
  const blockId = current?.dataset.blockId;
  if (!chapterId || !blockId) return;

  if (chapterId !== props.activeChapterId) emit("activeChapterChange", chapterId);
  const blockIndex = currentBook.blocks.findIndex((block) => block.id === blockId);
  emit("activeLocationChange", {
    chapterId,
    blockId,
    percentage: blockIndex <= 0 ? 0 : blockIndex / Math.max(1, currentBook.blocks.length - 1),
  });
}

function blockClass(block: ReaderBlock) {
  return {
    "reader-content__block": true,
    "reader-content__block--heading": block.kind === "heading",
    "reader-content__block--paragraph": block.kind === "paragraph",
    "reader-content__block--list": block.kind === "list-item",
    "reader-content__block--quote": block.kind === "blockquote",
    [`reader-content__block--h${block.level}`]: block.kind === "heading",
  };
}
</script>

<template>
  <main ref="scroller" class="reader-content kosmos-scroll" @scroll="onScroll">
    <section v-if="book" class="reader-content__page" :style="contentStyle">
      <h1 class="reader-content__book-title">{{ book.title }}</h1>
      <article
        v-for="block in book.blocks"
        :key="block.id"
        :class="blockClass(block)"
        :data-block-id="block.id"
        :data-chapter-id="block.chapterId"
      >
        <span
          v-for="(span, index) in block.spans"
          :key="`${block.id}-${index}`"
          :class="{
            'reader-content__span--strong': span.strong,
            'reader-content__span--emphasis': span.emphasis,
          }"
        >
          {{ span.text }}
        </span>
      </article>
    </section>

    <section v-else class="reader-content__empty">
      <BookOpen :size="42" aria-hidden="true" />
      <h1 class="reader-content__empty-title">Открой EPUB-книгу</h1>
      <p class="reader-content__empty-copy">
        Akasha покажет главы одним непрерывным потоком и сохранит настройки чтения.
      </p>
    </section>
  </main>
</template>
