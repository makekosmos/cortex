<script setup lang="ts">
import { computed, ref, watch } from "vue";
import { Modal, DateTimePicker } from "@kosmos/visuals";
import { Trash2 } from "@lucide/vue";
import type { TimeEntry } from "../types";
import { formatDuration } from "../lib/format";
import MentionInput from "./MentionInput.vue";

interface Props {
  open: boolean;
  entry: TimeEntry | null;
}

const props = defineProps<Props>();
const emit = defineEmits<{
  close: [];
  save: [
    patch: {
      id: string;
      title: string;
      startedAt: string;
      endedAt: string | null;
      taskId: string | null;
      taskTitle: string | null;
    },
  ];
  delete: [id: string];
}>();

const title = ref("");
const startedAtIso = ref<string | null>(null);
const endedAtIso = ref<string | null>(null);
const taskId = ref<string | null>(null);
const taskTitle = ref<string | null>(null);

watch(
  () => [props.open, props.entry?.id] as const,
  () => {
    if (props.entry && props.open) {
      title.value = props.entry.title;
      startedAtIso.value = props.entry.startedAt;
      endedAtIso.value = props.entry.endedAt;
      taskId.value = props.entry.taskId;
      taskTitle.value = props.entry.taskTitle;
    }
  },
  { immediate: true },
);

const duration = computed(() => {
  if (!startedAtIso.value) return 0;
  const start = new Date(startedAtIso.value).getTime();
  const end = endedAtIso.value ? new Date(endedAtIso.value).getTime() : Date.now();
  return Math.max(0, Math.floor((end - start) / 1000));
});

interface TitlePart {
  type: "text" | "task";
  text: string;
}

const titleParts = computed<TitlePart[]>(() => {
  if (!taskTitle.value) return [{ type: "text", text: title.value }];
  const needle = `@${taskTitle.value}`;
  const idx = title.value.indexOf(needle);
  if (idx < 0) return [{ type: "text", text: title.value }];
  const before = title.value.slice(0, idx);
  const after = title.value.slice(idx + needle.length);
  const parts: TitlePart[] = [];
  if (before) parts.push({ type: "text", text: before });
  parts.push({ type: "task", text: taskTitle.value });
  if (after) parts.push({ type: "text", text: after });
  return parts;
});

function save() {
  if (!props.entry) return;
  emit("save", {
    id: props.entry.id,
    title: title.value.trim() || "Без названия",
    startedAt: startedAtIso.value ?? props.entry.startedAt,
    endedAt: endedAtIso.value,
    taskId: taskId.value,
    taskTitle: taskTitle.value,
  });
}

function remove() {
  if (!props.entry) return;
  emit("delete", props.entry.id);
}
</script>

<template>
  <Modal :open="open" title="Редактирование записи" @close="emit('close')">
    <div v-if="entry" class="form">
      <div class="field">
        <span class="field__label">Описание</span>
        <div class="field__input-wrap">
          <MentionInput
            v-model="title"
            v-model:task-id="taskId"
            v-model:task-title="taskTitle"
            input-class="field__input-el"
            placeholder="@ для выбора задачи"
            @submit="save"
          />
        </div>
        <div v-if="taskTitle && titleParts.length > 0" class="field__preview">
          <template v-for="(p, i) in titleParts" :key="i">
            <span v-if="p.type === 'task'" class="pill-task">{{ p.text }}</span>
            <template v-else>{{ p.text }}</template>
          </template>
        </div>
      </div>

      <div class="time-row">
        <div class="time-field">
          <span class="field__label">С</span>
          <DateTimePicker v-model:value="startedAtIso" :reference="Date.now()" />
        </div>
        <span class="time-row__arrow">→</span>
        <div class="time-field">
          <span class="field__label">По</span>
          <DateTimePicker v-model:value="endedAtIso" :reference="startedAtIso" />
        </div>
        <div class="duration">{{ formatDuration(duration) }}</div>
      </div>
    </div>

    <template #footer>
      <button type="button" class="btn btn--danger" @click="remove">
        <Trash2 :size="14" :stroke-width="1.8" />
        Удалить
      </button>
      <div class="footer-spacer" />
      <button type="button" class="btn" @click="emit('close')">Отмена</button>
      <button type="button" class="btn btn--primary" @click="save">Сохранить</button>
    </template>
  </Modal>
</template>

<style scoped>
.form {
  display: flex;
  flex-direction: column;
  gap: 0.875rem;
}

.field {
  display: flex;
  flex-direction: column;
  gap: 0.25rem;
}

.field__label {
  font-size: 0.6875rem;
  text-transform: uppercase;
  letter-spacing: 0.06em;
  color: color-mix(in srgb, var(--foreground) 55%, transparent);
  font-weight: 600;
}

.field__input-wrap {
  height: 32px;
  padding: 0 0.625rem;
  background: color-mix(in srgb, var(--foreground) 4%, var(--background));
  border: 1px solid var(--border);
  border-radius: calc(var(--radius) * 0.7);
  corner-shape: var(--corner-shape);
}

.field__input-wrap:focus-within {
  border-color: color-mix(in srgb, var(--accent) 55%, transparent);
  background: var(--background);
}

:deep(.field__input-el) {
  height: 30px;
  font-size: 0.875rem;
}

.field__preview {
  display: flex;
  align-items: center;
  gap: 0.25rem;
  flex-wrap: wrap;
  padding: 0.125rem 0.625rem;
  font-size: 0.8125rem;
  line-height: 1.4;
  color: color-mix(in srgb, var(--foreground) 75%, transparent);
}

.pill-task {
  display: inline-flex;
  align-items: center;
  padding: 0 0.4rem;
  height: 18px;
  background: color-mix(in srgb, var(--accent) 18%, transparent);
  color: var(--accent);
  border-radius: 999px;
  font-weight: 600;
  font-size: 0.75rem;
}

.time-row {
  display: flex;
  align-items: flex-end;
  gap: 0.5rem;
}

.time-field {
  flex: 1;
  min-width: 0;
  display: flex;
  flex-direction: column;
  gap: 0.25rem;
}

.time-row__arrow {
  color: color-mix(in srgb, var(--foreground) 50%, transparent);
  padding-bottom: 0.5rem;
}

.duration {
  font-family: var(--font-mono);
  font-size: 0.875rem;
  font-variant-numeric: tabular-nums;
  font-weight: 600;
  min-width: 60px;
  text-align: right;
  color: var(--foreground);
  padding-bottom: 0.4rem;
}

.btn {
  display: inline-flex;
  align-items: center;
  gap: 0.4rem;
  height: 32px;
  padding: 0 0.875rem;
  background: transparent;
  border: 1px solid var(--border);
  border-radius: calc(var(--radius) * 0.7);
  corner-shape: var(--corner-shape);
  color: var(--foreground);
  font-size: 0.8125rem;
  font-weight: 500;
  cursor: pointer;
  font-family: inherit;
  transition: background-color 120ms cubic-bezier(0.2, 0, 0, 1);
}

.btn:hover {
  background: color-mix(in srgb, var(--foreground) 8%, transparent);
}

.btn--primary {
  background: var(--accent);
  border-color: var(--accent);
  color: var(--accent-foreground);
}

.btn--primary:hover {
  background: color-mix(in srgb, var(--accent) 90%, var(--foreground));
}

.btn--danger {
  border: none;
  background: transparent;
  color: var(--destructive);
}

.btn--danger:hover {
  background: color-mix(in srgb, var(--destructive) 12%, transparent);
}

.footer-spacer {
  flex: 1;
}
</style>
