<script setup lang="ts">
import type { EpubChapter } from "../lib/epub";

defineProps<{
  chapters: EpubChapter[];
  activeChapterId: string | null;
}>();

const emit = defineEmits<{
  jump: [chapterId: string];
}>();
</script>

<template>
  <aside class="reader-toc">
    <div class="reader-toc__header">Оглавление</div>
    <nav class="reader-toc__list kosmos-scroll" aria-label="Оглавление книги">
      <button
        v-for="chapter in chapters"
        :key="chapter.id"
        class="reader-toc__item"
        :class="{ 'reader-toc__item--active': chapter.id === activeChapterId }"
        type="button"
        @click="emit('jump', chapter.id)"
      >
        {{ chapter.title }}
      </button>
    </nav>
  </aside>
</template>
