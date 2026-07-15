<script setup lang="ts">
import { shallowRef } from "vue";
import { Upload } from "@lucide/vue";
import { Button } from "@kosmos/visuals";

withDefaults(
  defineProps<{
    disabled?: boolean;
    loading?: boolean;
  }>(),
  { disabled: false, loading: false },
);

const emit = defineEmits<{
  file: [file: File];
}>();

const input = shallowRef<HTMLInputElement | null>(null);
const dragging = shallowRef(false);

function chooseFile() {
  input.value?.click();
}

function acceptFile(file: File | undefined) {
  if (file) emit("file", file);
}

function handleInput(event: Event) {
  const target = event.target as HTMLInputElement;
  acceptFile(target.files?.[0]);
  target.value = "";
}

function handleDrop(event: DragEvent) {
  dragging.value = false;
  acceptFile(event.dataTransfer?.files[0]);
}

function handleKeydown(event: KeyboardEvent) {
  if (event.code !== "Enter" && event.code !== "Space") return;
  event.preventDefault();
  chooseFile();
}
</script>

<template>
  <div
    class="book-cover-dropzone"
    :class="dragging && 'is-dragging'"
    role="region"
    aria-label="Загрузка обложки книги"
    :aria-disabled="disabled || loading"
    :tabindex="disabled || loading ? -1 : 0"
    @click="chooseFile"
    @keydown="handleKeydown"
    @dragenter.prevent="dragging = true"
    @dragover.prevent="dragging = true"
    @dragleave.prevent="dragging = false"
    @drop.prevent="handleDrop"
  >
    <input
      ref="input"
      class="typed-object-header__cover-file-input"
      data-testid="book-cover-file-input"
      type="file"
      accept="image/*"
      tabindex="-1"
      :disabled="disabled || loading"
      @change="handleInput"
    />

    <span class="book-cover-dropzone__icon" aria-hidden="true">
      <Upload :size="22" />
    </span>
    <span class="book-cover-dropzone__title">Перетащи изображение сюда</span>
    <span class="book-cover-dropzone__hint">PNG, JPG или WebP до 10 МБ</span>
    <Button
      variant="ghost"
      size="sm"
      :loading="loading"
      :disabled="disabled"
      @click.stop="chooseFile"
    >
      Выбрать файл
    </Button>
  </div>
</template>

<style scoped>
.book-cover-dropzone {
  display: flex;
  min-height: 190px;
  flex-direction: column;
  align-items: center;
  justify-content: center;
  gap: var(--space-1);
  padding: var(--space-3);
  border: 2px dashed var(--border);
  border-radius: var(--radius-lg);
  outline: none;
  background: transparent;
  color: var(--foreground);
  text-align: center;
  transition:
    border-color 160ms var(--easing-standard),
    background-color 160ms var(--easing-standard);
}

.book-cover-dropzone:hover,
.book-cover-dropzone:focus-visible,
.book-cover-dropzone.is-dragging {
  border-color: color-mix(in srgb, var(--accent) 55%, var(--border));
  background: color-mix(in srgb, var(--accent) 7%, transparent);
}

.book-cover-dropzone[aria-disabled="true"] {
  pointer-events: none;
  opacity: 0.6;
}

.book-cover-dropzone__icon {
  display: grid;
  width: 44px;
  aspect-ratio: 1;
  place-items: center;
  margin-bottom: var(--space-1);
  border: 1px solid var(--border);
  border-radius: var(--radius-pill, 999px);
  color: var(--muted-foreground);
}

.book-cover-dropzone__title {
  font-size: var(--kosmos-text-body-size);
  font-weight: 600;
}

.book-cover-dropzone__hint {
  margin-bottom: var(--space-1);
  color: var(--muted-foreground);
  font-size: var(--kosmos-text-caption-size);
}

.typed-object-header__cover-file-input {
  position: absolute;
  width: 1px;
  height: 1px;
  overflow: hidden;
  clip: rect(0 0 0 0);
  clip-path: inset(50%);
  white-space: nowrap;
}
</style>
