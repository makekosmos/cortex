<script setup lang="ts">
import { computed, watch } from "vue";
import type { JSONContent } from "@tiptap/core";
import StarterKit from "@tiptap/starter-kit";
import { EditorContent, useEditor } from "@tiptap/vue-3";
import { plainTextToTiptapDoc } from "./bubbleDiaryModel";

defineOptions({ name: "BubbleTiptapRenderer" });

const props = defineProps<{
  contentJson?: JSONContent;
  fallbackText: string;
}>();

const content = computed(() => props.contentJson ?? plainTextToTiptapDoc(props.fallbackText));

const editor = useEditor({
  content: content.value,
  editable: false,
  extensions: [StarterKit],
  editorProps: {
    attributes: {
      class: "ProseMirror bubble-card__prosemirror",
      tabindex: "-1",
    },
  },
});

watch(
  content,
  (nextContent) => {
    editor.value?.commands.setContent(nextContent);
  },
  { deep: true },
);
</script>

<template>
  <EditorContent v-if="editor" :editor="editor" class="bubble-card__text bubble-rich-text" />
</template>

<style scoped>
.bubble-rich-text {
  flex: 1 1 auto;
  max-width: 62ch;
  min-width: 0;
  color: color-mix(in srgb, var(--foreground) 92%, transparent);
  cursor: default;
  font-size: 0.94rem;
  line-height: 1.5;
}

.bubble-rich-text :deep(.ProseMirror) {
  min-height: 0 !important;
  outline: none;
  letter-spacing: 0;
}

.bubble-rich-text :deep(.ProseMirror p) {
  margin: 0;
}

.bubble-rich-text :deep(.ProseMirror > *) {
  margin-top: 0;
  margin-bottom: 0.45rem;
}

.bubble-rich-text :deep(.ProseMirror > :last-child) {
  margin-bottom: 0;
}

.bubble-rich-text :deep(.ProseMirror code) {
  border-radius: 4px;
  background: color-mix(in srgb, var(--foreground) 7%, transparent);
  padding: 0.05rem 0.25rem;
  font-family: var(--font-mono, ui-monospace, monospace);
  font-size: 0.9em;
}

.bubble-rich-text :deep(.ProseMirror pre) {
  overflow-x: auto;
  max-width: 100%;
  border-radius: 8px;
  background: color-mix(in srgb, var(--foreground) 6%, transparent);
  padding: 0.7rem 0.8rem;
  font-family: var(--font-mono, ui-monospace, monospace);
  font-size: 0.86rem;
  line-height: 1.55;
}

.bubble-rich-text :deep(.ProseMirror pre code) {
  background: transparent;
  padding: 0;
}
</style>
