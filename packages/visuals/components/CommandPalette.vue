<script setup lang="ts">
import { nextTick, ref, watch } from "vue";

const props = defineProps<{
  open: boolean;
  placeholder?: string;
  query?: string;
  dialogTestId?: string;
  inputTestId?: string;
}>();

const emit = defineEmits<{
  "update:open": [boolean];
  "update:query": [string];
}>();

const query = ref(props.query ?? "");
const inputRef = ref<HTMLInputElement>();
const listRef = ref<HTMLElement>();

watch(
  () => props.query,
  (value) => {
    if (typeof value !== "string" || value === query.value) {
      return;
    }

    query.value = value;
  },
);

watch(
  () => props.open,
  (value) => {
    if (!value) {
      return;
    }

    query.value = props.query ?? "";
    nextTick(() => inputRef.value?.focus());
  },
);

function close() {
  emit("update:open", false);
}

function updateQuery(value: string) {
  query.value = value;
  emit("update:query", value);
}

function handleInputKeydown(event: KeyboardEvent) {
  if (event.key === "Escape") {
    close();
    return;
  }

  if (event.key === "ArrowDown") {
    event.preventDefault();
    const items = listRef.value?.querySelectorAll<HTMLElement>("[data-cmd-item]");
    if (items?.length) {
      items[0].focus();
    }
  }
}

function handleListKeydown(event: KeyboardEvent) {
  const items = [...(listRef.value?.querySelectorAll<HTMLElement>("[data-cmd-item]") ?? [])];
  const index = items.indexOf(document.activeElement as HTMLElement);

  if (event.key === "ArrowDown") {
    event.preventDefault();
    items[(index + 1) % items.length]?.focus();
    return;
  }

  if (event.key === "ArrowUp") {
    event.preventDefault();
    if (index <= 0) {
      inputRef.value?.focus();
    } else {
      items[index - 1].focus();
    }
    return;
  }

  if (event.key === "Escape") {
    close();
    return;
  }

  if (event.key.length === 1) {
    inputRef.value?.focus();
  }
}
</script>

<template>
  <div v-if="open" class="fixed inset-0 z-50 flex items-start justify-center pt-32">
    <div class="absolute inset-0 bg-black/40 backdrop-blur-sm" @click="close" />

    <div
      class="relative z-10 w-full max-w-screen-sm overflow-hidden rounded-xl border border-(--border) shadow-2xl"
      style="background: var(--color-shape-highlight-light-solid)"
      :data-testid="dialogTestId"
    >
      <div class="flex items-center gap-4 px-4 py-4">
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
          :value="query"
          :placeholder="placeholder ?? 'Поиск...'"
          class="flex-1 bg-transparent text-sm text-(--foreground) outline-none placeholder:text-(--muted-foreground)"
          :data-testid="inputTestId"
          @input="updateQuery(($event.target as HTMLInputElement).value)"
          @keydown="handleInputKeydown"
        />
        <kbd
          class="hidden h-6 rounded border border-(--border) px-2 text-[10px] leading-6 text-(--muted-foreground) sm:block"
        >
          esc
        </kbd>
      </div>

      <div class="h-px bg-(--border)" />

      <div ref="listRef" class="max-h-96 overflow-y-auto py-2" @keydown="handleListKeydown">
        <slot :query="query" :close="close" />
      </div>
    </div>
  </div>
</template>
