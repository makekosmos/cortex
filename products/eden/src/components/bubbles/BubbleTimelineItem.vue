<template>
  <article
    class="bubble-timeline-item"
    :class="{
      'bubble-timeline-item--reply': Boolean(node.parentId),
      'bubble-timeline-item--continues-thread': continuesThread || replyOpen,
    }"
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
            :style="{
              '--bubble-kind-color': option?.color ?? bubbleKindColor(node.kind),
            }"
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
    </div>
    <div class="bubble-timeline-item__content">
      <section class="bubble-card" :aria-label="`Запись ${occurrenceLabel}`">
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
              {{ occurrenceLabel }}
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

  <div
    v-if="replyTargetId"
    class="bubble-thread-actions"
    :class="{
      'bubble-thread-actions--active': threadActive,
      'bubble-thread-actions--replying': replyOpen,
    }"
  >
    <button
      v-if="!replyOpen"
      class="bubble-thread-actions__reply"
      type="button"
      :data-testid="`bubble-reply-${replyTargetId}`"
      @click="replyOpen = true"
    >
      <svg aria-hidden="true" viewBox="0 0 100 100">
        <path
          d="M78,50A30,30,0,1,0,48,80l1,0v0H82V72H68.37A29.92,29.92,0,0,0,78,50Zm-8,0a21.89,21.89,0,0,1-1.42,7.78,62.34,62.34,0,0,0-6.44-7.26L69,43.61A21.92,21.92,0,0,1,70,50ZM65.13,36.21,56,45.35a61.75,61.75,0,0,0-7.27-4.64l9.47-10.2A22.14,22.14,0,0,1,65.13,36.21ZM48,28c.52,0,1,0,1.56.06L41,37.22a62.36,62.36,0,0,0-8.77-2.59A21.93,21.93,0,0,1,48,28Zm0,44A22,22,0,0,1,27.57,58.14,38.06,38.06,0,0,1,49.22,72C48.81,72,48.41,72,48,72Zm9.53-2.17A46.07,46.07,0,0,0,26,49.71a21.87,21.87,0,0,1,1.56-7.86A54,54,0,0,1,64,65.1,22.11,22.11,0,0,1,57.53,69.83Z"
        />
      </svg>
      Ответить
    </button>
  </div>

  <form
    v-if="replyTargetId && replyOpen"
    class="bubble-reply-draft"
    :data-testid="`bubble-reply-composer-${replyTargetId}`"
    @submit.prevent="submitReply"
  >
    <div class="bubble-reply-draft__rail" aria-hidden="true">
      <span class="bubble-reply-draft__dot" />
    </div>
    <div class="bubble-reply-draft__content">
      <textarea
        v-model="replyText"
        rows="2"
        :aria-label="`Ответ в ветку ${occurrenceLabel}`"
        :data-testid="`bubble-reply-input-${replyTargetId}`"
      />
      <div class="bubble-reply-draft__actions">
        <button type="button" @click="cancelReply">Отмена</button>
        <button type="submit" :disabled="!canSubmitReply">Ответить</button>
      </div>
    </div>
  </form>
</template>

<script setup lang="ts">
import { computed, nextTick, ref, watch } from "vue";
import { Dropdown } from "@kosmos/visuals";
import BubbleTiptapRenderer from "./BubbleTiptapRenderer.vue";
import type { BubbleKind, BubbleTimelineNode } from "./bubbleDiaryModel";
import {
  BUBBLE_KIND_OPTIONS,
  bubbleDateKey,
  formatBubbleOccurrenceLabel,
  normalizeBubbleKind,
  parseBubbleDraft,
} from "./bubbleDiaryModel";

defineOptions({ name: "BubbleTimelineItem" });

const props = defineProps<{
  node: BubbleTimelineNode;
  now: number;
  continuesThread?: boolean;
  replyTargetId?: string;
  threadActive?: boolean;
}>();

const emit = defineEmits<{
  update: [id: string, input: string];
  remove: [id: string];
  "kind-change": [id: string, kind: BubbleKind];
  reply: [parentId: string, input: string];
}>();

const isEditing = ref(false);
const editText = ref(draftFromNode(props.node));
const editInputRef = ref<HTMLTextAreaElement | null>(null);
const deleteStep = ref(0);
const replyOpen = ref(false);
const replyText = ref("");
const canCommitEdit = computed(() => parseBubbleDraft(editText.value).text.length > 0);
const canSubmitReply = computed(() => parseBubbleDraft(replyText.value).text.length > 0);
const occurrenceLabel = computed(() =>
  formatBubbleOccurrenceLabel(
    props.node.createdAt ?? props.node.sortKey ?? props.node.time,
    new Date(props.now),
  ),
);

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

function cancelReply(): void {
  replyOpen.value = false;
  replyText.value = "";
}

function submitReply(): void {
  if (!canSubmitReply.value || !props.replyTargetId) return;
  emit("reply", props.replyTargetId, replyText.value);
  cancelReply();
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
  --bubble-thread-line: var(--border-color-strong, var(--border));
  position: relative;
  display: flex;
  min-width: 0;
  padding: 0.25rem 0 0;
  transition:
    color 120ms ease,
    opacity 120ms ease;
  scroll-margin-top: 3rem;
}

.bubble-timeline-item--reply::before,
.bubble-timeline-item--continues-thread::after {
  position: absolute;
  left: calc(0.625rem - 1px);
  width: 2px;
  background: var(--bubble-thread-line);
  content: "";
  pointer-events: none;
}

.bubble-timeline-item--reply::before {
  top: 0;
  height: 1.175rem;
}

.bubble-timeline-item--continues-thread::after {
  top: 1.175rem;
  bottom: 0;
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
  position: relative;
  z-index: 1;
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

.bubble-thread-actions {
  --bubble-thread-line: var(--border-color-strong, var(--border));
  position: relative;
  display: flex;
  height: 40px;
  min-height: 40px;
  align-items: flex-start;
  margin-top: -10px;
  padding: 0 16px 0 1.5rem;
}

.bubble-thread-actions--replying::before {
  position: absolute;
  top: 0;
  bottom: 0;
  left: calc(0.625rem - 1px);
  width: 2px;
  background: var(--bubble-thread-line);
  content: "";
}

.bubble-thread-actions__reply {
  display: inline-flex;
  align-items: center;
  gap: 0.25rem;
  border: 0;
  border-radius: var(--radius-pill, 999px);
  background: transparent;
  padding: 2px 10px 2px 3px;
  color: transparent;
  font-size: 0.9rem;
  line-height: 1;
  opacity: 0;
  pointer-events: none;
  transform: translateY(-3px);
  transition:
    background-color 180ms ease,
    color 220ms ease,
    opacity 220ms ease,
    transform 220ms ease;
  transition-delay: 0ms;
}

.bubble-thread-actions__reply svg {
  width: 23px;
  height: 23px;
  fill: currentColor;
}

.bubble-thread-actions--active .bubble-thread-actions__reply,
.bubble-thread-actions:focus-within .bubble-thread-actions__reply {
  color: var(--muted-foreground);
  opacity: 1;
  pointer-events: auto;
  transform: translateY(0);
}

.bubble-thread-actions__reply:hover,
.bubble-thread-actions__reply:focus-visible {
  background: color-mix(in srgb, var(--foreground) 7%, transparent);
  color: var(--foreground);
  transition-delay: 0ms;
}

.bubble-reply-draft {
  --bubble-thread-line: var(--border-color-strong, var(--border));
  position: relative;
  display: flex;
  min-width: 0;
  padding: 0.25rem 0 0;
}

.bubble-reply-draft::before {
  position: absolute;
  top: 0;
  left: calc(0.625rem - 1px);
  width: 2px;
  height: 1.175rem;
  background: var(--bubble-thread-line);
  content: "";
}

.bubble-reply-draft__rail {
  display: flex;
  width: 1.25rem;
  flex: 0 0 1.25rem;
  justify-content: center;
  margin-right: 0.75rem;
  padding-top: 0.3rem;
}

.bubble-reply-draft__dot {
  position: relative;
  z-index: 1;
  width: 1.25rem;
  height: 1.25rem;
  border-radius: var(--radius-pill, 999px);
  background: var(--bubble-thread-line);
}

.bubble-reply-draft__content {
  display: grid;
  min-width: 0;
  flex: 1 1 auto;
  gap: 0.4rem;
  padding: 0.25rem 0.45rem 0.95rem 0;
}

.bubble-reply-draft textarea {
  min-height: 3.4rem;
  resize: vertical;
  border: 1px solid var(--border);
  border-radius: var(--radius-sm, 6px);
  background: color-mix(in srgb, var(--foreground) 4%, transparent);
  padding: 0.45rem 0.55rem;
  color: var(--foreground);
  font: inherit;
}

.bubble-reply-draft__actions {
  display: flex;
  justify-content: flex-end;
  gap: 0.4rem;
}

.bubble-reply-draft__actions button {
  min-height: 28px;
  border: 1px solid var(--border);
  border-radius: var(--radius-sm, 6px);
  padding: 0 0.6rem;
  color: var(--foreground);
  font-size: 0.75rem;
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
