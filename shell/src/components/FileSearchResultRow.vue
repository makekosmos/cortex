<script setup lang="ts">
import { computed } from "vue";
import type { Component } from "vue";
import {
  File,
  FileArchive,
  FileAudio,
  FileCode2,
  FileImage,
  FileSpreadsheet,
  FileText,
  FileVideo,
} from "@lucide/vue";

const props = defineProps<{
  title?: string;
  path: string;
  selected: boolean;
}>();

const emit = defineEmits<{
  select: [];
}>();

const displayTitle = computed(() => {
  const title = props.title?.trim();
  if (title) return title;
  return props.path.split(/[\\/]/).filter(Boolean).at(-1)?.trim() || "Без названия";
});

const extension = computed(() => {
  const dot = displayTitle.value.lastIndexOf(".");
  return dot > 0 ? displayTitle.value.slice(dot + 1).toLowerCase() : "";
});

const fileIcon = computed<Component>(() => {
  if (
    [
      "c",
      "cc",
      "cpp",
      "cs",
      "css",
      "go",
      "html",
      "java",
      "js",
      "json",
      "jsx",
      "kt",
      "mjs",
      "py",
      "rs",
      "sh",
      "sql",
      "ts",
      "tsx",
      "vue",
      "xml",
      "yaml",
      "yml",
    ].includes(extension.value)
  ) {
    return FileCode2;
  }
  if (["csv", "ods", "xls", "xlsx"].includes(extension.value)) return FileSpreadsheet;
  if (["gif", "jpeg", "jpg", "png", "svg", "webp"].includes(extension.value)) return FileImage;
  if (["flac", "m4a", "mp3", "ogg", "wav"].includes(extension.value)) return FileAudio;
  if (["avi", "mkv", "mov", "mp4", "webm"].includes(extension.value)) return FileVideo;
  if (["7z", "gz", "rar", "tar", "zip"].includes(extension.value)) return FileArchive;
  if (["doc", "docx", "md", "pdf", "rtf", "txt"].includes(extension.value)) return FileText;
  return File;
});
</script>

<template>
  <li class="result file-row" :class="{ selected: props.selected }" @click="emit('select')">
    <span class="file-row__icon" aria-hidden="true">
      <component :is="fileIcon" :size="19" :stroke-width="1.8" />
    </span>
    <span class="file-row__title" :title="displayTitle">{{ displayTitle }}</span>
    <span class="file-row__path" :title="props.path">{{ props.path }}</span>
    <span class="file-row__kind">Файл</span>
  </li>
</template>

<style scoped>
.file-row {
  display: flex;
  align-items: center;
  gap: 12px;
  padding: 10px 14px;
  border: 1px solid transparent;
  border-radius: 6px;
  background: transparent;
  cursor: pointer;
  margin: 1px 0;
}

.file-row:hover {
  background: color-mix(in srgb, oklch(1 0 0) 3.5%, transparent);
}

.file-row.selected {
  background: color-mix(in srgb, oklch(1 0 0) 8%, transparent);
  border-color: color-mix(in srgb, oklch(1 0 0) 12%, transparent);
}

.file-row__icon {
  display: inline-flex;
  align-items: center;
  justify-content: center;
  width: 22px;
  height: 22px;
  color: color-mix(in srgb, var(--foreground) 78%, oklch(0.72 0.1 225));
}

.file-row__title,
.file-row__path {
  min-width: 0;
  overflow: hidden;
  white-space: nowrap;
  text-overflow: ellipsis;
}

.file-row__title {
  flex: 0 0 auto;
  max-width: calc(100% - 180px);
  color: var(--foreground);
  font-size: 14px;
  font-weight: 600;
}

.file-row__path,
.file-row__kind {
  font-size: 12px;
}

.file-row__path {
  flex: 1 1 0;
  color: color-mix(in srgb, var(--foreground) 50%, transparent);
}

.file-row__kind {
  flex-shrink: 0;
  color: color-mix(in srgb, var(--foreground) 32%, transparent);
  padding-left: 4px;
}
</style>
