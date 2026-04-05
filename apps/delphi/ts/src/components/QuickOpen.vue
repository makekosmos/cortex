<script setup lang="ts">
/* eslint-disable no-console */
import {
  computed,
  nextTick,
  onMounted,
  onUnmounted,
  shallowRef,
  useTemplateRef,
  watch,
} from "vue";
import { CheckCircle, Folder, Search, Tag } from "lucide-vue-next";
import { useTodoStore } from "@/store/todos";
import { storeToRefs } from "pinia";
import type { Project, Tag as TagType, TodoItem } from "@/types/task";

// ---------------------------------------------------------------------------
// Result types
// ---------------------------------------------------------------------------

type QuickOpenResult =
  | { kind: "todo"; item: TodoItem }
  | { kind: "project"; item: Project }
  | { kind: "tag"; item: TagType };

function resultId(r: QuickOpenResult): string {
  return `${r.kind}-${r.item.id}`;
}

// ---------------------------------------------------------------------------
// Fuzzy search (simple substring / token match, case-insensitive)
// ---------------------------------------------------------------------------

function fuzzyScore(text: string, query: string): number {
  const lower = text.toLowerCase();
  const q = query.toLowerCase();

  if (lower.includes(q)) return 1;

  const queryTokens = q.split(/\s+/).filter(Boolean);
  if (
    queryTokens.length > 1 &&
    queryTokens.every((tok) => lower.includes(tok))
  ) {
    return 0.8;
  }

  const textTokens = lower.split(/\s+/);
  if (queryTokens.some((qt) => textTokens.some((tt) => tt.startsWith(qt)))) {
    return 0.5;
  }

  return 0;
}

// ---------------------------------------------------------------------------
// State
// ---------------------------------------------------------------------------

const open = shallowRef(false);
const searchText = shallowRef("");
const debouncedQuery = shallowRef("");
const selectedIndex = shallowRef(0);

const inputRef = useTemplateRef<HTMLInputElement>("searchInput");

const store = useTodoStore();
const { todos, projects, tags } = storeToRefs(store);

function close() {
  open.value = false;
  searchText.value = "";
  debouncedQuery.value = "";
  selectedIndex.value = 0;
}

// Cmd+K to toggle
onMounted(() => {
  const handler = (e: KeyboardEvent) => {
    if (e.metaKey && e.key === "k") {
      e.preventDefault();
      if (open.value) {
        searchText.value = "";
        debouncedQuery.value = "";
        selectedIndex.value = 0;
        open.value = false;
      } else {
        open.value = true;
      }
    }
  };
  window.addEventListener("keydown", handler);
  onUnmounted(() => window.removeEventListener("keydown", handler));
});

// Focus on open
watch(open, (val) => {
  if (val) {
    nextTick(() => inputRef.value?.focus());
  }
});

// Debounce search text (200ms)
let debounceTimer: ReturnType<typeof setTimeout> | undefined;
watch(searchText, (val) => {
  if (debounceTimer) clearTimeout(debounceTimer);
  debounceTimer = setTimeout(() => {
    debouncedQuery.value = val;
    selectedIndex.value = 0;
  }, 200);
});

// Compute results
const results = computed<QuickOpenResult[]>(() => {
  if (!debouncedQuery.value.trim()) return [];
  const q = debouncedQuery.value.trim();
  const items: QuickOpenResult[] = [];

  // Todos (top 8)
  const scoredTodos = todos.value
    .filter((t) => !t.isTrashed)
    .map((t) => {
      const titleScore = fuzzyScore(t.title, q);
      const notesScore = t.notes ? fuzzyScore(t.notes, q) * 0.8 : 0;
      return { todo: t, score: Math.max(titleScore, notesScore) };
    })
    .filter((x) => x.score > 0)
    .toSorted((a, b) => b.score - a.score)
    .slice(0, 8);

  items.push(
    ...scoredTodos.map((x) => ({ kind: "todo" as const, item: x.todo })),
  );

  // Projects (top 3)
  const matchedProjects = projects.value
    .filter((p) => fuzzyScore(p.title, q) > 0)
    .slice(0, 3);
  items.push(
    ...matchedProjects.map((p) => ({ kind: "project" as const, item: p })),
  );

  // Tags (top 3)
  const matchedTags = tags.value
    .filter((t) => fuzzyScore(t.title, q) > 0)
    .slice(0, 3);
  items.push(...matchedTags.map((t) => ({ kind: "tag" as const, item: t })));

  return items;
});

function activateSelected() {
  if (selectedIndex.value >= results.value.length) return;
  const result = results.value[selectedIndex.value];

  if (result.kind === "todo") {
    console.log("Selected todo:", result.item.id);
  } else if (result.kind === "project") {
    console.log("Selected project:", result.item.id);
  } else if (result.kind === "tag") {
    console.log("Selected tag:", result.item.id);
  }

  close();
}

function handleKeyDown(e: KeyboardEvent) {
  if (e.key === "ArrowDown") {
    e.preventDefault();
    selectedIndex.value = Math.min(
      results.value.length - 1,
      selectedIndex.value + 1,
    );
  } else if (e.key === "ArrowUp") {
    e.preventDefault();
    selectedIndex.value = Math.max(0, selectedIndex.value - 1);
  } else if (e.key === "Enter") {
    e.preventDefault();
    activateSelected();
  } else if (e.key === "Escape") {
    e.preventDefault();
    close();
  }
}

function getResultSubtitle(result: QuickOpenResult): string {
  switch (result.kind) {
    case "todo": {
      const todo = result.item as TodoItem;
      if (todo.isCompleted) return "Завершена";
      if (todo.projectId) {
        const project = projects.value.find((p) => p.id === todo.projectId);
        return project?.title ?? "Входящие";
      }
      return "Входящие";
    }
    case "project": {
      const project = result.item as Project;
      const count = todos.value.filter(
        (t) => t.projectId === project.id && !t.isTrashed,
      ).length;
      return `${count} задач`;
    }
    case "tag": {
      const tag = result.item;
      const count = todos.value.filter((t) => t.tagIds.includes(tag.id)).length;
      return `${count} задач`;
    }
  }
}
</script>

<template>
  <div
    v-if="open"
    class="fixed inset-0 z-50 flex items-start justify-center pt-[18vh]"
  >
    <!-- Backdrop -->
    <div class="absolute inset-0 bg-black/40" @click="close" />

    <!-- Panel -->
    <div
      class="relative z-10 w-[480px] overflow-hidden rounded-xl border border-(--border) bg-(--popover) shadow-2xl"
    >
      <!-- Search input -->
      <div class="flex items-center gap-2.5 px-4 py-3.5">
        <Search :size="16" class="shrink-0 text-(--muted-foreground)" />
        <input
          ref="searchInput"
          type="text"
          placeholder="Поиск задач, проектов, тегов..."
          :value="searchText"
          class="w-full text-base text-(--foreground) placeholder:text-(--muted-foreground)"
          @input="searchText = ($event.target as HTMLInputElement).value"
          @keydown="handleKeyDown"
        />
      </div>

      <!-- Results -->
      <template v-if="results.length > 0">
        <div class="h-px bg-(--border) opacity-50" />
        <div class="max-h-[280px] overflow-y-auto py-1">
          <div
            v-for="(result, index) in results"
            :key="resultId(result)"
            :class="[
              'flex cursor-pointer items-center gap-2.5 px-4 py-2',
              index === selectedIndex ? 'bg-blue-500/10' : '',
            ]"
            @click="
              selectedIndex = index;
              activateSelected();
            "
            @mouseenter="selectedIndex = index"
          >
            <div class="shrink-0">
              <CheckCircle
                v-if="result.kind === 'todo'"
                :size="16"
                class="text-blue-500"
              />
              <Folder
                v-else-if="result.kind === 'project'"
                :size="16"
                class="text-purple-500"
              />
              <Tag v-else :size="16" class="text-orange-500" />
            </div>
            <div class="min-w-0 flex-1">
              <div class="truncate text-sm text-(--foreground)">
                {{ result.item.title }}
              </div>
              <div class="truncate text-xs text-(--muted-foreground)">
                {{ getResultSubtitle(result) }}
              </div>
            </div>
          </div>
        </div>
      </template>

      <!-- Empty state -->
      <div
        v-if="results.length === 0 && debouncedQuery.trim() !== ''"
        class="px-4 py-3.5 text-center text-sm text-(--muted-foreground)"
      >
        Ничего не найдено
      </div>
    </div>
  </div>
</template>
