<script setup lang="ts">
import { computed, nextTick, ref } from "vue";
import type { DelphiTask } from "../types";
import { tasks as tasksRef, loadTasksOnce, ensureFreshTasks } from "../lib/store";
import type { PomodoroDraftTask } from "../lib/store";
import MentionMenu from "./MentionMenu.vue";

interface Props {
  modelValue: string;
  tasks: PomodoroDraftTask[];
  placeholder?: string;
}

const props = withDefaults(defineProps<Props>(), {
  placeholder: "Над чем работаем? @ для задачи",
});

const emit = defineEmits<{
  "update:modelValue": [v: string];
  "update:tasks": [v: PomodoroDraftTask[]];
  submit: [];
}>();

const inputRef = ref<HTMLInputElement | null>(null);
const menuRef = ref<InstanceType<typeof MentionMenu> | null>(null);

const open = ref(false);
const query = ref("");
const anchor = ref(0);
const highlight = ref(0);

void loadTasksOnce();

const tasksSrc = computed<DelphiTask[]>(() => tasksRef.value);

const draft = computed({
  get: () => props.modelValue,
  set: (v) => emit("update:modelValue", v),
});

function onInput() {
  const el = inputRef.value;
  if (!el) return;
  const caret = el.selectionStart ?? draft.value.length;
  let a = -1;
  for (let i = caret - 1; i >= 0; i--) {
    const ch = draft.value[i];
    if (ch === "@") {
      const prev = i === 0 ? " " : draft.value[i - 1];
      if (/\s/.test(prev) || i === 0) a = i;
      break;
    }
    if (/\s/.test(ch)) break;
  }
  if (a >= 0) {
    const q = draft.value.slice(a + 1, caret);
    if (/^[\p{L}\p{N}_\- ]*$/u.test(q)) {
      anchor.value = a;
      query.value = q;
      open.value = true;
      highlight.value = 0;
      void ensureFreshTasks();
      return;
    }
  }
  open.value = false;
}

function pickTask(task: DelphiTask) {
  if (props.tasks.some((t) => t.id === task.id)) {
    open.value = false;
    removeMentionQuery();
    return;
  }
  emit("update:tasks", [...props.tasks, { id: task.id, title: task.title }]);
  removeMentionQuery();
  open.value = false;
}

function removeMentionQuery() {
  const el = inputRef.value;
  if (!el) return;
  const caret = el.selectionStart ?? draft.value.length;
  const before = draft.value.slice(0, anchor.value);
  const after = draft.value.slice(caret);
  draft.value = (before + after).trimStart();
  nextTick(() => {
    const newEl = inputRef.value;
    if (!newEl) return;
    const pos = before.length;
    newEl.focus();
    newEl.setSelectionRange(pos, pos);
  });
}

function removeTask(index: number) {
  const next = props.tasks.slice();
  next.splice(index, 1);
  emit("update:tasks", next);
}

function onKeyDown(e: KeyboardEvent) {
  if (open.value) {
    const filtered = menuRef.value?.filtered ?? [];
    if (e.key === "ArrowDown") {
      e.preventDefault();
      highlight.value = Math.min(highlight.value + 1, filtered.length - 1);
      return;
    }
    if (e.key === "ArrowUp") {
      e.preventDefault();
      highlight.value = Math.max(0, highlight.value - 1);
      return;
    }
    if (e.key === "Enter") {
      const pick = filtered[highlight.value];
      if (pick) {
        e.preventDefault();
        pickTask(pick);
        return;
      }
    }
    if (e.key === "Escape") {
      e.preventDefault();
      open.value = false;
      return;
    }
  }
  if (e.key === "Backspace" && !open.value && draft.value === "" && props.tasks.length > 0) {
    e.preventDefault();
    const next = props.tasks.slice(0, -1);
    emit("update:tasks", next);
    return;
  }
  if (e.key === "Enter" && !open.value) {
    e.preventDefault();
    emit("submit");
  }
}
</script>

<template>
  <div class="pdi">
    <div class="pdi__row">
      <button
        v-for="(t, i) in tasks"
        :key="t.id"
        type="button"
        class="pdi__chip"
        :aria-label="`Убрать ${t.title}`"
        @click="removeTask(i)"
      >
        <span class="pdi__chip-label">{{ t.title }}</span>
      </button>
      <input
        ref="inputRef"
        v-model="draft"
        class="pdi__input"
        :placeholder="tasks.length === 0 ? placeholder : ''"
        @input="onInput"
        @keydown="onKeyDown"
      />
    </div>
    <MentionMenu
      ref="menuRef"
      :open="open"
      :query="query"
      :tasks="tasksSrc"
      :highlighted-index="highlight"
      @pick="pickTask"
      @hover="(i) => (highlight = i)"
    />
  </div>
</template>

<style scoped>
.pdi {
  position: relative;
  width: 100%;
}

.pdi__row {
  display: flex;
  flex-wrap: wrap;
  align-items: center;
  gap: 0.3rem;
  min-height: 48px;
  padding: 0.375rem 0.75rem;
  background: var(--background);
  border: 2px solid var(--border);
  border-radius: calc(var(--radius) * 2);
  corner-shape: var(--corner-shape);
  transition: border-color 160ms cubic-bezier(0.2, 0, 0, 1);
}

.pdi__row:focus-within {
  border-color: color-mix(in srgb, var(--accent) 55%, transparent);
}

.pdi__chip {
  display: inline-flex;
  align-items: center;
  height: 28px;
  padding: 0 0.625rem;
  background: color-mix(in srgb, var(--accent) 22%, transparent);
  color: var(--accent);
  border: none;
  border-radius: 999px;
  font-family: inherit;
  font-size: 0.8125rem;
  font-weight: 600;
  max-width: 140px;
  user-select: none;
  -webkit-user-select: none;
  transition: background-color 120ms cubic-bezier(0.2, 0, 0, 1);
}

.pdi__chip:hover {
  background: color-mix(in srgb, var(--accent) 32%, transparent);
}

.pdi__chip-label {
  overflow: hidden;
  text-overflow: ellipsis;
  white-space: nowrap;
}

.pdi__input {
  flex: 1;
  min-width: 80px;
  height: 32px;
  background: transparent;
  border: none;
  outline: none;
  color: var(--foreground);
  font-family: inherit;
  font-size: 1rem;
  font-weight: 600;
  text-align: left;
}

.pdi__input::placeholder {
  color: color-mix(in srgb, var(--foreground) 45%, transparent);
}
</style>
