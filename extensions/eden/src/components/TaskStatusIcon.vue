<script setup lang="ts">
// TaskStatusIcon — 18×18 SVG-иконка для одного из 7 Linear-style статусов
// задачи. Заменяет Checkbox в TaskRef NodeView. Click → next status в цикле
// (todo → done → todo); полный picker через right-click ContextMenu.
//
// Visual conventions:
//   triage      — dashed outline circle + ? glyph (нужно отсортировать)
//   backlog     — dotted outline circle (когда-нибудь потом)
//   todo        — solid outline circle (готово к работе, активная)
//   done        — filled solid + ✓ (terminal success)
//   canceled    — filled muted + × (terminal not-done)
//
// Цвет акцента переопределяется через CSS var `--task-status-accent` на
// родителе (Eden подсовывает свой orange).

import { computed } from "vue";
import type { TaskStatus } from "@/lib/taskStatus";

interface Props {
  status: TaskStatus;
  disabled?: boolean;
  ariaLabel?: string;
}

const props = withDefaults(defineProps<Props>(), {
  disabled: false,
});

const emit = defineEmits<{
  click: [];
}>();

const isCompleted = computed(() => props.status === "done");
const isTerminated = computed(() => props.status === "canceled");
const dim = computed(() => isCompleted.value || isTerminated.value);

function onClick(e: MouseEvent) {
  if (props.disabled) return;
  e.stopPropagation();
  emit("click");
}
</script>

<template>
  <button
    type="button"
    role="checkbox"
    :aria-checked="isCompleted"
    :aria-label="ariaLabel"
    :disabled="disabled"
    class="task-status-icon"
    :class="[
      `task-status-icon--${status}`,
      { 'task-status-icon--dim': dim, 'task-status-icon--disabled': disabled },
    ]"
    @click="onClick"
  >
    <svg width="18" height="18" viewBox="0 0 18 18" fill="none" aria-hidden="true">
      <!-- triage: dashed outline + ? glyph -->
      <template v-if="status === 'triage'">
        <rect
          x="1.5"
          y="1.5"
          width="15"
          height="15"
          rx="6"
          stroke="currentColor"
          stroke-width="2"
          stroke-dasharray="2 2"
        />
        <text x="9" y="13" text-anchor="middle" font-size="10" font-weight="600" fill="currentColor">?</text>
      </template>

      <!-- backlog: dotted outline -->
      <template v-else-if="status === 'backlog'">
        <rect
          x="1.5"
          y="1.5"
          width="15"
          height="15"
          rx="6"
          stroke="currentColor"
          stroke-width="2"
          stroke-dasharray="1 2"
        />
      </template>

      <!-- todo: solid outline (Delphi unchecked parity) -->
      <template v-else-if="status === 'todo'">
        <rect
          x="1.5"
          y="1.5"
          width="15"
          height="15"
          rx="6"
          stroke="currentColor"
          stroke-width="2"
        />
      </template>

      <!-- done: filled accent + check glyph (Delphi checked parity-ish) -->
      <template v-else-if="status === 'done'">
        <rect x="1.5" y="1.5" width="15" height="15" rx="6" fill="currentColor" />
        <rect x="3.5" y="3.5" width="11" height="11" rx="3" fill="var(--task-status-fill-inner, currentColor)" />
      </template>

      <!-- canceled: filled muted + × -->
      <template v-else-if="status === 'canceled'">
        <rect x="1.5" y="1.5" width="15" height="15" rx="6" fill="currentColor" />
        <path
          d="M6 6 L12 12 M12 6 L6 12"
          stroke="var(--task-status-glyph, #fff)"
          stroke-width="2"
          stroke-linecap="round"
        />
      </template>
    </svg>
  </button>
</template>

<style scoped>
.task-status-icon {
  display: inline-flex;
  align-items: center;
  justify-content: center;
  width: 18px;
  height: 18px;
  padding: 0;
  background: transparent;
  border: none;
  cursor: pointer;
  flex-shrink: 0;
  color: var(--ring);
  transition: color 0.15s ease, opacity 0.15s ease;
}

.task-status-icon:hover:not(.task-status-icon--disabled) {
  color: var(--task-status-accent, var(--accent));
}

/* Active statuses — нейтральный outline, hover → accent. */
.task-status-icon--todo,
.task-status-icon--triage,
.task-status-icon--backlog {
  color: var(--ring);
}

/* Done — accent fill. Inner derives white via --task-status-fill-inner. */
.task-status-icon--done {
  color: var(--task-status-accent, var(--accent));
}

/* Canceled — muted gray fill. */
.task-status-icon--canceled {
  color: var(--muted-foreground, #888);
}

/* Затемнение для всех терминальных статусов (done/canceled/duplicate) —
   касается самого квадратика, не только title. */
.task-status-icon--dim {
  opacity: 0.55;
}

.task-status-icon--disabled {
  cursor: not-allowed;
  opacity: 0.4;
}

.task-status-icon:focus-visible {
  outline: 2px solid var(--task-status-accent, var(--accent));
  outline-offset: 2px;
  border-radius: 6px;
}
</style>
