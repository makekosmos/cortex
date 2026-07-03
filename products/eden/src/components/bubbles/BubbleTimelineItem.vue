<template>
  <article
    class="bubble-timeline-item"
    :data-testid="`bubble-node-${node.id}`"
    :data-bubble-date="bubbleDateKey(node)"
    :style="{ '--bubble-source-accent': bubbleKindColor(node.kind) }"
  >
    <div class="bubble-timeline-item__rail">
      <Dropdown
        class="bubble-kind-dropdown"
        :data-testid="`bubble-kind-${node.id}`"
        :model-value="node.kind"
        :options="BUBBLE_KIND_OPTIONS"
        :match-trigger-width="false"
        :max-height-px="180"
        :searchable="false"
        :show-chevron="false"
        panel-align="start"
        @update:model-value="(kind) => emit('kind-change', node.id, kind)"
      >
        <template #trigger-leading="{ option }">
          <span
            class="bubble-kind-dropdown__dot"
            :style="{ '--bubble-kind-color': option?.color ?? bubbleKindColor(node.kind) }"
            aria-hidden="true"
          />
        </template>
        <template #option-leading="{ option }">
          <span
            class="bubble-kind-dropdown__option-dot"
            :style="{ '--bubble-kind-color': option.color }"
            aria-hidden="true"
          />
        </template>
      </Dropdown>
      <span class="bubble-timeline-item__line" aria-hidden="true" />
    </div>
    <div class="bubble-timeline-item__content">
      <section class="bubble-card" :aria-label="`Запись ${node.time}`">
        <template v-if="!isEditing">
          <div class="bubble-card__line">
            <BubbleTiptapRenderer :content-json="node.contentJson" :fallback-text="node.text" />
            <button
              class="bubble-card__time"
              type="button"
              title="Редактировать запись"
              :data-testid="`bubble-time-${node.id}`"
              @click="startEditing"
            >
              {{ node.time }}
            </button>
          </div>

          <div
            v-if="node.tags.length > 0"
            class="bubble-card__tags"
            :data-testid="`bubble-tags-${node.id}`"
          >
            <span v-for="tag in node.tags" :key="tag" class="bubble-card__tag">{{ tag }}</span>
          </div>
        </template>

        <div v-else class="bubble-card__edit" :data-testid="`bubble-editor-${node.id}`">
          <textarea
            ref="editInputRef"
            v-model="editText"
            class="bubble-card__edit-input"
            :data-testid="`bubble-edit-input-${node.id}`"
            rows="2"
            @keydown.ctrl.enter.prevent="commitEdit"
            @keydown.meta.enter.prevent="commitEdit"
            @keydown.esc.prevent="cancelEditing"
          />

          <div class="bubble-card__edit-actions">
            <button class="bubble-card__delete" type="button" @click="requestDelete">
              {{ deleteStep === 0 ? "Удалить" : "Точно удалить" }}
            </button>
            <button class="bubble-card__cancel" type="button" @click="cancelEditing">Отмена</button>
            <button
              class="bubble-card__update"
              type="button"
              :disabled="!canCommitEdit"
              @click="commitEdit"
            >
              Обновить
            </button>
          </div>
        </div>
      </section>
    </div>
  </article>
</template>

<script setup lang="ts">
import { computed, nextTick, ref, watch } from "vue";
import { Dropdown } from "@kosmos/visuals";
import BubbleTiptapRenderer from "./BubbleTiptapRenderer.vue";
import type { BubbleKind, BubbleTimelineNode } from "./bubbleDiaryModel";
import {
  BUBBLE_KIND_OPTIONS,
  bubbleDateKey,
  normalizeBubbleKind,
  parseBubbleDraft,
} from "./bubbleDiaryModel";

defineOptions({ name: "BubbleTimelineItem" });

const props = defineProps<{
  node: BubbleTimelineNode;
}>();

const emit = defineEmits<{
  update: [id: string, input: string];
  remove: [id: string];
  "kind-change": [id: string, kind: BubbleKind];
}>();

const isEditing = ref(false);
const editText = ref(draftFromNode(props.node));
const editInputRef = ref<HTMLTextAreaElement | null>(null);
const deleteStep = ref(0);
const canCommitEdit = computed(() => parseBubbleDraft(editText.value).text.length > 0);

watch(
  () => props.node,
  (node) => {
    if (!isEditing.value) editText.value = draftFromNode(node);
  },
);

function draftFromNode(node: BubbleTimelineNode): string {
  return [node.text, ...node.tags.map((tag) => `#${tag}`)].join(" ");
}

function startEditing(): void {
  isEditing.value = true;
  deleteStep.value = 0;
  editText.value = draftFromNode(props.node);
  void nextTick(() => editInputRef.value?.focus());
}

function cancelEditing(): void {
  isEditing.value = false;
  deleteStep.value = 0;
  editText.value = draftFromNode(props.node);
}

function commitEdit(): void {
  if (!canCommitEdit.value) return;

  emit("update", props.node.id, editText.value);
  isEditing.value = false;
  deleteStep.value = 0;
}

function requestDelete(): void {
  if (deleteStep.value === 0) {
    deleteStep.value = 1;
    return;
  }

  emit("remove", props.node.id);
}

function bubbleKindColor(kind: BubbleKind): string {
  return (
    BUBBLE_KIND_OPTIONS.find((option) => option.value === normalizeBubbleKind(kind))?.color ??
    BUBBLE_KIND_OPTIONS[0].color
  );
}
</script>

<style scoped>
.bubble-timeline-item {
  --bubble-source-accent: var(--border-color-strong, var(--border));
  display: flex;
  min-width: 0;
  padding: 0.25rem 0 0;
  transition:
    color 120ms ease,
    opacity 120ms ease;
  scroll-margin-top: 3rem;
}

.bubble-timeline-item__rail {
  display: flex;
  width: 1.25rem;
  flex: 0 0 1.25rem;
  flex-direction: column;
  align-items: center;
  margin-right: 0.75rem;
  padding-top: 0.3rem;
  user-select: none;
}

.bubble-kind-dropdown {
  width: 1.25rem;
  height: 1.25rem;
  flex: 0 0 1.25rem;
}

.bubble-kind-dropdown :deep(button) {
  width: 1.25rem;
  min-width: 1.25rem;
  height: 1.25rem;
  justify-content: center;
  overflow: hidden;
  border: 0;
  border-radius: var(--radius-pill, 999px);
  background: transparent;
  padding: 0;
}

.bubble-kind-dropdown :deep(button > span:not(.bubble-kind-dropdown__dot)) {
  position: absolute;
  width: 1px;
  height: 1px;
  overflow: hidden;
  clip-path: inset(50%);
  white-space: nowrap;
}

.bubble-kind-dropdown__dot {
  display: block;
  width: 100%;
  height: 100%;
  border-radius: var(--radius-pill, 999px);
  background: var(--bubble-kind-color);
  transition:
    transform 120ms ease,
    filter 120ms ease;
}

.bubble-kind-dropdown:hover .bubble-kind-dropdown__dot,
.bubble-kind-dropdown:focus-within .bubble-kind-dropdown__dot {
  filter: contrast(0.86);
}

.bubble-kind-dropdown:active .bubble-kind-dropdown__dot {
  transform: scale(0.9);
}

.bubble-kind-dropdown__option-dot {
  display: block;
  width: 0.875rem;
  height: 0.875rem;
  flex: 0 0 0.875rem;
  border-radius: var(--radius-pill, 999px);
  background: var(--bubble-kind-color);
}

.bubble-timeline-item__line {
  width: 2px;
  flex: 1 0 0;
  min-height: 1.3rem;
  margin-top: 0.2rem;
  background: color-mix(in srgb, var(--bubble-source-accent) 50%, var(--border));
  opacity: 0.72;
  transition: opacity 120ms ease;
}

.bubble-timeline-item:last-child .bubble-timeline-item__line {
  opacity: 0;
}

.bubble-timeline-item__content {
  min-width: 0;
  flex: 1 1 auto;
  padding: 0 0 0.95rem;
}

.bubble-card {
  min-width: 0;
  border-radius: 8px;
  padding: 0.25rem 0.45rem 0.35rem 0;
}

.bubble-card__line {
  display: flex;
  min-width: 0;
  align-items: baseline;
  gap: 0.75rem;
  color: var(--foreground);
}

.bubble-card__time {
  flex: 0 0 auto;
  margin-left: auto;
  border: 0;
  border-radius: 6px;
  background: transparent;
  padding: 0.18rem 0.32rem;
  color: color-mix(in srgb, var(--foreground) 46%, transparent);
  font-family: var(--font-mono, ui-monospace, monospace);
  font-size: 0.72rem;
  font-weight: 600;
  line-height: 1.2;
  transition:
    background-color 120ms ease,
    color 120ms ease;
}

.bubble-card__time:hover,
.bubble-card__time:focus-visible {
  background: color-mix(in srgb, var(--foreground) 7%, transparent);
  color: color-mix(in srgb, var(--foreground) 82%, transparent);
}

.bubble-card__text {
  flex: 1 1 auto;
  max-width: 62ch;
  min-width: 0;
  margin: 0;
  color: color-mix(in srgb, var(--foreground) 92%, transparent);
  cursor: default;
  font-size: 0.94rem;
  line-height: 1.5;
}

.bubble-card__tags {
  display: flex;
  min-width: 0;
  flex-wrap: wrap;
  align-items: center;
  gap: 0.35rem;
  margin-top: 0.5rem;
}

.bubble-card__tag {
  display: inline-flex;
  max-width: 100%;
  align-items: center;
  border: 1px solid color-mix(in srgb, var(--bubble-source-accent) 30%, var(--border));
  border-radius: var(--radius-pill, 999px);
  background: color-mix(in srgb, var(--bubble-source-accent) 9%, transparent);
  padding: 0.12rem 0.48rem;
  color: color-mix(in srgb, var(--foreground) 86%, transparent);
  font-size: 0.72rem;
  font-weight: 600;
  line-height: 1.45;
}

.bubble-card__edit {
  display: flex;
  min-width: 0;
  flex-direction: column;
  gap: 0.5rem;
}

.bubble-card__edit-input {
  width: 100%;
  min-height: 4.25rem;
  resize: vertical;
  border: 1px solid var(--border);
  border-radius: 8px;
  background: color-mix(in srgb, var(--foreground) 4%, transparent);
  padding: 0.55rem 0.65rem;
  color: var(--foreground);
  font: inherit;
  line-height: 1.45;
  outline: none;
}

.bubble-card__edit-input:focus {
  border-color: color-mix(in srgb, var(--accent) 58%, var(--border));
}

.bubble-card__edit-actions {
  display: flex;
  justify-content: flex-end;
  gap: 0.4rem;
}

.bubble-card__delete,
.bubble-card__cancel,
.bubble-card__update {
  min-height: 30px;
  border: 1px solid var(--border);
  border-radius: 7px;
  padding: 0 0.65rem;
  font-size: 0.78rem;
  font-weight: 650;
}

.bubble-card__delete {
  margin-right: auto;
  color: color-mix(in srgb, var(--danger, var(--accent)) 78%, var(--foreground));
}

.bubble-card__cancel {
  color: color-mix(in srgb, var(--foreground) 72%, transparent);
}

.bubble-card__update {
  border-color: transparent;
  background: var(--accent);
  color: var(--accent-foreground, var(--background));
}

.bubble-card__update:disabled {
  opacity: 0.45;
}

@media (max-width: 720px) {
  .bubble-timeline-item__rail {
    margin-right: 0.6rem;
  }
}
</style>
