<script setup lang="ts">
import { useRouter } from "vue-router";
import { FolderOpen } from "lucide-vue-next";
import { storeToRefs } from "pinia";
import { CommandPalette, EmptyState, TodoRow } from "@kepler/visuals";
import { useTodoStore } from "@/store/todos";

const props = defineProps<{ open: boolean }>();
const emit = defineEmits<{ "update:open": [boolean] }>();

const router = useRouter();
const store = useTodoStore();
const { todos, projects } = storeToRefs(store);

function matchTodos(query: string) {
  if (!query.trim()) return [];
  const q = query.toLowerCase();
  return todos.value
    .filter((t) => !t.isTrashed && t.title.toLowerCase().includes(q))
    .slice(0, 8);
}

function matchProjects(query: string) {
  if (!query.trim()) return [];
  const q = query.toLowerCase();
  return projects.value
    .filter((p) => p.title.toLowerCase().includes(q))
    .slice(0, 5);
}

async function selectTodo(
  todo: { id: string; projectId?: string | null },
  close: () => void,
) {
  close();
  const target = todo.projectId ? `/project/${todo.projectId}` : "/";
  await router.push(target);
  setTimeout(() => {
    const el = document.querySelector(
      `[data-todo-id="${todo.id}"]`,
    ) as HTMLElement | null;
    if (!el) return;
    el.scrollIntoView({ behavior: "smooth", block: "center" });

    const siblings =
      el.parentElement?.querySelectorAll<HTMLElement>("[data-todo-id]") ?? [];
    for (const sib of siblings) {
      sib.style.transition = "opacity 0.5s ease, box-shadow 0.5s ease";
      sib.style.opacity = sib === el ? "1" : "0.25";
    }
    el.style.position = "relative";
    el.style.zIndex = "10";
    el.style.boxShadow =
      "0 0 0 1px var(--accent), 0 0 8px 0 var(--accent)";
    el.style.borderRadius = "var(--radius)";

    setTimeout(() => {
      for (const sib of siblings) {
        sib.style.opacity = "1";
      }
      el.style.boxShadow = "none";
      setTimeout(() => {
        for (const sib of siblings) {
          sib.style.transition = "";
          sib.style.opacity = "";
        }
        el.style.position = "";
        el.style.zIndex = "";
        el.style.boxShadow = "";
        el.style.borderRadius = "";
      }, 500);
    }, 2000);
  }, 150);
}

function selectProject(project: { id: string }, close: () => void) {
  close();
  router.push(`/project/${project.id}`);
}
</script>

<template>
  <CommandPalette
    :open="open"
    placeholder="Поиск задач и проектов..."
    @update:open="emit('update:open', $event)"
  >
    <template #default="{ query, close }">
      <!-- Empty state -->
      <EmptyState
        v-if="!query.trim()"
        compact
        title="Начните вводить для поиска"
      />

      <!-- No results -->
      <EmptyState
        v-else-if="
          matchTodos(query).length === 0 && matchProjects(query).length === 0
        "
        compact
        title="Ничего не найдено"
        :description="`По запросу «${query}» совпадений нет`"
      />


      <template v-else>
        <!-- Todos -->
        <div v-if="matchTodos(query).length > 0" class="px-2">
          <TodoRow
            v-for="todo in matchTodos(query)"
            :key="todo.id"
            :todo="todo"
            :draggable="false"
            :editable="false"
            data-cmd-item
            @click="selectTodo(todo, close)"
            @complete="store.completeTodo(todo.id)"
          />
        </div>

        <!-- Projects -->
        <template v-if="matchProjects(query).length > 0">
          <div
            class="px-3 pb-1 pt-2 text-[10px] font-semibold uppercase tracking-wider text-(--muted-foreground)/60 select-none"
          >
            Проекты
          </div>
          <button
            v-for="project in matchProjects(query)"
            :key="project.id"
            data-cmd-item
            class="flex w-full items-center gap-3 px-3 py-2 text-sm text-(--foreground) outline-none hover:bg-(--surface) focus:bg-(--surface)"
            @click="selectProject(project, close)"
          >
            <FolderOpen :size="14" class="shrink-0 text-(--muted-foreground)" />
            <span class="min-w-0 truncate">{{ project.title }}</span>
          </button>
        </template>
      </template>
    </template>
  </CommandPalette>
</template>
