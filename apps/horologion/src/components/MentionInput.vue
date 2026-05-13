<script setup lang="ts">
import { computed, nextTick, ref, watch } from "vue";
import type { DelphiTask } from "@shared/ipc-types";
import { tasks as tasksRef, loadTasksOnce } from "../lib/store";
import MentionMenu from "./MentionMenu.vue";

interface Props {
  modelValue: string;
  taskId: string | null;
  taskTitle: string | null;
  placeholder?: string;
  /** CSS-класс для input'а (стилизация передаётся из родителя). */
  inputClass?: string;
  /** Автофокус при mount. */
  autofocus?: boolean;
}

const props = withDefaults(defineProps<Props>(), {
  placeholder: "@ для выбора задачи",
  inputClass: undefined,
  autofocus: false,
});

const emit = defineEmits<{
  "update:modelValue": [v: string];
  "update:taskId": [v: string | null];
  "update:taskTitle": [v: string | null];
  /** Enter в input'е (когда меню закрыто). */
  submit: [];
}>();

const inputRef = ref<HTMLInputElement | null>(null);
const menuRef = ref<InstanceType<typeof MentionMenu> | null>(null);

const open = ref(false);
const query = ref("");
const anchor = ref(0);
const highlight = ref(0);

void loadTasksOnce();

const tasks = computed<DelphiTask[]>(() => tasksRef.value);

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
      return;
    }
  }
  open.value = false;
}

function pickTask(task: DelphiTask) {
  const before = draft.value.slice(0, anchor.value);
  const caret = inputRef.value?.selectionStart ?? draft.value.length;
  const after = draft.value.slice(caret);
  const inserted = `@${task.title} `;
  draft.value = before + inserted + after;
  emit("update:taskId", task.id);
  emit("update:taskTitle", task.title);
  open.value = false;
  nextTick(() => {
    const el = inputRef.value;
    if (!el) return;
    const pos = (before + inserted).length;
    el.focus();
    el.setSelectionRange(pos, pos);
  });
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
  if (e.key === "Enter" && !open.value) {
    e.preventDefault();
    emit("submit");
  }
}

if (props.autofocus) {
  watch(
    inputRef,
    (el) => {
      if (el) el.focus();
    },
    { immediate: true },
  );
}

defineExpose({ focus: () => inputRef.value?.focus() });
</script>

<template>
  <div class="mention-input">
    <input
      ref="inputRef"
      v-model="draft"
      :class="['mention-input__el', inputClass]"
      :placeholder="placeholder"
      @input="onInput"
      @keydown="onKeyDown"
    />
    <MentionMenu
      ref="menuRef"
      :open="open"
      :query="query"
      :tasks="tasks"
      :highlighted-index="highlight"
      @pick="pickTask"
      @hover="(i) => (highlight = i)"
    />
  </div>
</template>

<style scoped>
.mention-input {
  position: relative;
  display: flex;
  align-items: center;
  width: 100%;
}

.mention-input__el {
  flex: 1;
  min-width: 0;
  background: transparent;
  border: none;
  outline: none;
  color: var(--foreground);
  font-family: inherit;
  font-size: inherit;
}
</style>
