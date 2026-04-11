<script setup lang="ts">
import { ref, watch, nextTick } from "vue";

const props = defineProps<{
  open: boolean;
  placeholder?: string;
}>();

const emit = defineEmits<{
  "update:open": [boolean];
}>();

const query = ref("");
const inputRef = ref<HTMLInputElement>();
const listRef = ref<HTMLElement>();

watch(
  () => props.open,
  (val) => {
    if (val) {
      query.value = "";
      nextTick(() => inputRef.value?.focus());
    }
  },
);

function close() {
  emit("update:open", false);
}

function handleInputKeydown(e: KeyboardEvent) {
  if (e.key === "Escape") {
    close();
    return;
  }
  if (e.key === "ArrowDown") {
    e.preventDefault();
    const items = listRef.value?.querySelectorAll<HTMLElement>("[data-cmd-item]");
    if (items?.length) items[0].focus();
  }
}

function handleListKeydown(e: KeyboardEvent) {
  const items = [
    ...(listRef.value?.querySelectorAll<HTMLElement>("[data-cmd-item]") ?? []),
  ];
  const idx = items.indexOf(document.activeElement as HTMLElement);

  if (e.key === "ArrowDown") {
    e.preventDefault();
    items[(idx + 1) % items.length]?.focus();
  } else if (e.key === "ArrowUp") {
    e.preventDefault();
    if (idx <= 0) {
      inputRef.value?.focus();
    } else {
      items[idx - 1].focus();
    }
  } else if (e.key === "Escape") {
    close();
  } else if (e.key.length === 1) {
    inputRef.value?.focus();
  }
}
</script>

<template>
  <div
    v-if="open"
    class="fixed inset-0 z-50 flex items-start justify-center pt-[20vh]"
  >
    <!-- Backdrop -->
    <div class="absolute inset-0 bg-black/40 backdrop-blur-sm" @click="close" />

    <!-- Dialog -->
    <div
      class="relative z-10 w-full max-w-(--bringhurst-wide) overflow-hidden rounded-xl border border-(--border) shadow-2xl "
      style="background: var(--color-shape-highlight-light-solid)"
    >
      <!-- Input -->
      <div class="flex items-center gap-3 px-4 py-3">
        <svg
          xmlns="http://www.w3.org/2000/svg"
          width="16"
          height="16"
          viewBox="0 0 24 24"
          fill="none"
          stroke="currentColor"
          stroke-width="2"
          stroke-linecap="round"
          stroke-linejoin="round"
          class="shrink-0 text-(--muted-foreground)"
        >
          <circle cx="11" cy="11" r="8" />
          <path d="m21 21-4.3-4.3" />
        </svg>
        <input
          ref="inputRef"
          v-model="query"
          :placeholder="placeholder ?? 'Поиск...'"
          class="flex-1 bg-transparent text-sm text-(--foreground) outline-none placeholder:text-(--muted-foreground)"
          @keydown="handleInputKeydown"
        />
        <kbd class="hidden rounded border border-(--border) px-1.5 py-0.5 text-[10px] text-(--muted-foreground) sm:block">
          esc
        </kbd>
      </div>

      <div class="h-px bg-(--border)" />

      <!-- Results -->
      <div
        ref="listRef"
        class="max-h-96 overflow-y-auto py-2"
        @keydown="handleListKeydown"
      >
        <slot :query="query" :close="close" />
      </div>
    </div>
  </div>
</template>
