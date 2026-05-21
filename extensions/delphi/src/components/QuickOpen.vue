<script setup lang="ts">
/* eslint-disable no-console */
import { computed, onMounted, onUnmounted, shallowRef } from "vue";
import { CheckCircle, Folder, Tag } from "lucide-vue-next";
import { storeToRefs } from "pinia";
import { CommandPalette, EmptyState } from "@kosmos/visuals";
import { useTodoStore } from "@/store/todos";
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
  if (queryTokens.length > 1 && queryTokens.every((tok) => lower.includes(tok))) {
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

const store = useTodoStore();
const { todos, projects, tags } = storeToRefs(store);

// Cmd+K to toggle
let handler: ((e: KeyboardEvent) => void) | undefined;

onMounted(() => {
  handler = (e: KeyboardEvent) => {
    if ((e.metaKey || e.ctrlKey) && e.key === "k") {
      e.preventDefault();
      open.value = !open.value;
    }
  };
  window.addEventListener("keydown", handler);
});

onUnmounted(() => {
  if (handler) window.removeEventListener("keydown", handler);
});

function computeResults(query: string): QuickOpenResult[] {
  if (!query.trim()) return [];
  const q = query.trim();
  const items: QuickOpenResult[] = [];

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

  items.push(...scoredTodos.map((x) => ({ kind: "todo" as const, item: x.todo })));

  const matchedProjects = projects.value.filter((p) => fuzzyScore(p.title, q) > 0).slice(0, 3);
  items.push(...matchedProjects.map((p) => ({ kind: "project" as const, item: p })));

  const matchedTags = tags.value.filter((t) => fuzzyScore(t.title, q) > 0).slice(0, 3);
  items.push(...matchedTags.map((t) => ({ kind: "tag" as const, item: t })));

  return items;
}

function activate(result: QuickOpenResult, close: () => void) {
  if (result.kind === "todo") {
    console.log("Selected todo:", result.item.id);
  } else if (result.kind === "project") {
    console.log("Selected project:", result.item.id);
  } else if (result.kind === "tag") {
    console.log("Selected tag:", result.item.id);
  }
  close();
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
      const count = todos.value.filter((t) => t.projectId === project.id && !t.isTrashed).length;
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
  <CommandPalette
    :open="open"
    placeholder="Поиск задач, проектов, тегов..."
    @update:open="open = $event"
  >
    <template #default="{ query, close }">
      <template v-if="computeResults(query).length > 0">
        <button
          v-for="result in computeResults(query)"
          :key="resultId(result)"
          data-cmd-item
          class="flex w-full items-center gap-2.5 px-4 py-2 text-left outline-none hover:bg-(--surface) focus:bg-(--surface)"
          @click="activate(result, close)"
        >
          <div class="shrink-0">
            <CheckCircle v-if="result.kind === 'todo'" :size="16" class="text-(--primary)" />
            <Folder
              v-else-if="result.kind === 'project'"
              :size="16"
              class="text-(--muted-foreground)"
            />
            <Tag v-else :size="16" class="text-(--muted-foreground)" />
          </div>
          <div class="min-w-0 flex-1">
            <div class="truncate text-sm text-(--foreground)">
              {{ result.item.title }}
            </div>
            <div class="truncate text-xs text-(--muted-foreground)">
              {{ getResultSubtitle(result) }}
            </div>
          </div>
        </button>
      </template>

      <EmptyState v-else-if="query.trim() !== ''" compact title="Ничего не найдено" />
    </template>
  </CommandPalette>
</template>
