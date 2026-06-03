<script setup lang="ts">
import ReaderContent from "./ReaderContent.vue";
import ReaderToc from "./ReaderToc.vue";
import type { EpubBook } from "../lib/epub";
import type { ReaderSettings } from "../composables/useEpubReader";

defineProps<{
  activeBlockId: string | null;
  activeChapterId: string | null;
  book: EpubBook | null;
  settings: ReaderSettings;
}>();

const emit = defineEmits<{
  activeChapterChange: [chapterId: string];
  activeLocationChange: [location: { chapterId: string; blockId: string; percentage: number }];
  jump: [chapterId: string];
}>();
</script>

<template>
  <div class="akasha-reader">
    <div class="akasha-shell__body">
      <ReaderToc
        v-if="book"
        :chapters="book.chapters"
        :active-chapter-id="activeChapterId"
        @jump="emit('jump', $event)"
      />
      <ReaderContent
        :book="book"
        :active-block-id="activeBlockId"
        :active-chapter-id="activeChapterId"
        :font-family="settings.fontFamily"
        :font-size="settings.fontSize"
        @active-chapter-change="emit('activeChapterChange', $event)"
        @active-location-change="emit('activeLocationChange', $event)"
      />
    </div>
  </div>
</template>
