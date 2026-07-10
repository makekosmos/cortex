<script setup lang="ts">
import { computed, onBeforeUnmount, onMounted, ref, watch } from "vue";
import type { JSONContent } from "@tiptap/core";
import Placeholder from "@tiptap/extension-placeholder";
import StarterKit from "@tiptap/starter-kit";
import { EditorContent, useEditor } from "@tiptap/vue-3";
import { useVirtualizer } from "@tanstack/vue-virtual";
import type { VirtualItem } from "@tanstack/vue-virtual";
import { PhArrowUp } from "@phosphor-icons/vue";
import BubbleDiaryCalendarSidebar from "./BubbleDiaryCalendarSidebar.vue";
import BubbleTimelineItem from "./BubbleTimelineItem.vue";
import { edenApi } from "@/lib/edenApi";
import {
  LOCAL_BUBBLES_STORAGE_KEY,
  LOCAL_BUBBLES_STORAGE_VERSION,
  bubbleOccurrenceMillis,
  createDraftBubble,
  createJournalBubblesFromEntry,
  isLegacyDatedJournalEntry,
  normalizeLocalBubbles,
  parseBubbleDraft,
  type BubbleKind,
  type BubbleTimelineNode,
} from "./bubbleDiaryModel";

const props = defineProps<{
  journalEntries?: Entry[];
  calendarOpen?: boolean;
}>();
const emit = defineEmits<{
  journalMigrated: [];
}>();

const draftPlainText = ref("");
const localBubbles = ref<BubbleTimelineNode[]>([]);
const labelNow = ref(Date.now());
const activeThreadId = ref<string>();
const timelineRef = ref<HTMLElement | null>(null);
let journalCleanupRunning = false;
let stopBubbleChanges: (() => void) | undefined;
let midnightTimer: ReturnType<typeof setTimeout> | undefined;
let activeBubbleWrites = 0;

const draftPreview = computed(() => parseBubbleDraft(draftPlainText.value));
const canSubmitDraft = computed(() => draftPreview.value.text.length > 0);
const rowVirtualizer = useVirtualizer<HTMLElement, HTMLElement>(
  computed(() => ({
    count: localBubbles.value.length,
    getScrollElement: () => timelineRef.value,
    estimateSize: () => 64,
    overscan: 8,
    useAnimationFrameWithResizeObserver: true,
    getItemKey: (index) => localBubbles.value[index]?.id ?? index,
  })),
);
const virtualRows = computed<
  Array<{
    row: VirtualItem;
    node: BubbleTimelineNode;
    threadRootId: string;
    continuesThread: boolean;
    replyTargetId?: string;
  }>
>(() =>
  rowVirtualizer.value.getVirtualItems().flatMap((row) => {
    const node = localBubbles.value[row.index];
    const nextNode = localBubbles.value[row.index + 1];
    const threadRootId = node?.parentId ?? node?.id;
    const continuesThread = nextNode?.parentId === threadRootId;
    return node && threadRootId
      ? [
          {
            row,
            node,
            threadRootId,
            continuesThread,
            replyTargetId: continuesThread ? undefined : threadRootId,
          },
        ]
      : [];
  }),
);
const virtualListHeight = computed(() => `${rowVirtualizer.value.getTotalSize()}px`);

const composerEditor = useEditor({
  content: "",
  extensions: [
    StarterKit.configure({
      codeBlock: {
        enableTabIndentation: true,
        tabSize: 2,
      },
    }),
    Placeholder.configure({ placeholder: "Напиши мысль..." }),
  ],
  editorProps: {
    attributes: {
      class: "ProseMirror bubble-composer__prosemirror",
      "data-testid": "bubble-composer-input",
      "aria-label": "Новая мысль",
      spellcheck: "true",
    },
    handleKeyDown(_view, event) {
      if ((event.ctrlKey || event.metaKey) && event.key === "Enter") {
        event.preventDefault();
        addDraftBubble();
        return true;
      }

      return false;
    },
  },
  onUpdate({ editor }) {
    draftPlainText.value = editor.getText();
  },
});

onMounted(() => {
  void startDiary();
  window.addEventListener("focus", refreshLabels);
  document.addEventListener("visibilitychange", refreshLabelsWhenVisible);
  scheduleMidnightRefresh();
});

onBeforeUnmount(() => {
  stopBubbleChanges?.();
  if (midnightTimer) clearTimeout(midnightTimer);
  window.removeEventListener("focus", refreshLabels);
  document.removeEventListener("visibilitychange", refreshLabelsWhenVisible);
});

watch(
  () => props.journalEntries,
  () => {
    void migrateJournalEntries().then(refreshLocalBubbles);
  },
);

async function addDraftBubble(): Promise<void> {
  const editor = composerEditor.value;
  if (!editor) return;

  const bubble = createDraftBubble(editor.getJSON() as JSONContent, new Date(), editor.getText());
  if (!bubble) return;

  try {
    await edenApi.createBubble(editor.getText(), bubble.kind, undefined, bubble.contentJson);
    editor.commands.clearContent();
    draftPlainText.value = "";
    await refreshLocalBubbles();
  } catch (err) {
    console.warn("[eden] bubble create failed:", err);
  }
}

function focusComposer(event: MouseEvent): void {
  const target = event.target;
  if (target instanceof Element && target.closest("button")) return;
  composerEditor.value?.commands.focus("end");
}

async function updateLocalBubble(id: string, input: string): Promise<void> {
  await runBubbleWrite(() => edenApi.updateBubble(id, { input }));
}

async function deleteLocalBubble(id: string): Promise<void> {
  await runBubbleWrite(() => edenApi.deleteBubble(id));
}

async function updateBubbleKind(id: string, kind: BubbleKind): Promise<void> {
  await runBubbleWrite(() => edenApi.updateBubble(id, { kind }));
}

async function replyToBubble(parentId: string, input: string): Promise<void> {
  await runBubbleWrite(() => edenApi.createBubble(input, "plain", parentId));
}

async function runBubbleWrite(write: () => Promise<unknown>): Promise<void> {
  activeBubbleWrites += 1;
  try {
    await write();
  } catch (err) {
    console.warn("[eden] bubble write failed:", err);
  } finally {
    activeBubbleWrites -= 1;
    if (activeBubbleWrites === 0) await refreshLocalBubbles();
  }
}

function activateThread(threadId: string): void {
  activeThreadId.value = threadId;
}

function deactivateThread(threadId: string, event: MouseEvent | FocusEvent): void {
  const nextTarget = event.relatedTarget;
  const nextThreadId =
    nextTarget instanceof Element
      ? nextTarget.closest<HTMLElement>("[data-thread-id]")?.dataset.threadId
      : undefined;
  if (nextThreadId !== threadId && activeThreadId.value === threadId) {
    activeThreadId.value = undefined;
  }
}

function scrollToDate(date: string): void {
  const index = localBubbles.value.findIndex((bubble) => bubble.date === date);
  if (index < 0) return;
  rowVirtualizer.value.scrollToIndex(index, {
    align: "center",
    behavior: "smooth",
  });
}

function measureVirtualRow(element: Element | null): void {
  if (element instanceof HTMLElement) rowVirtualizer.value.measureElement(element);
}

async function migrateLocalBubbles(): Promise<void> {
  try {
    const raw = JSON.parse(localStorage.getItem(LOCAL_BUBBLES_STORAGE_KEY) ?? "null");
    const record = raw && typeof raw === "object" ? (raw as Record<string, unknown>) : null;
    if (record?.version !== LOCAL_BUBBLES_STORAGE_VERSION || !Array.isArray(record.bubbles)) return;

    const remaining: unknown[] = [];
    for (const source of record.bubbles) {
      const bubble = normalizeLocalBubbles([source])[0];
      if (!bubble || bubbleOccurrenceMillis(bubble) === null) {
        remaining.push(source);
        continue;
      }
      try {
        await edenApi.migrateBubble(
          "local-storage-v1",
          `${bubble.id}:${JSON.stringify(source)}`,
          bubble,
        );
      } catch (err) {
        remaining.push(source);
        console.warn("[eden] local bubble migration incomplete:", err);
      }
    }

    if (remaining.length === 0) localStorage.removeItem(LOCAL_BUBBLES_STORAGE_KEY);
    else
      localStorage.setItem(
        LOCAL_BUBBLES_STORAGE_KEY,
        JSON.stringify({ ...record, bubbles: remaining }),
      );
  } catch (err) {
    console.warn("[eden] local bubble migration source is unreadable:", err);
  }
}

async function refreshLocalBubbles(): Promise<void> {
  try {
    localBubbles.value = await edenApi.listBubbles();
  } catch (err) {
    console.warn("[eden] bubble load failed; keeping rendered data:", err);
  }
}

async function startDiary(): Promise<void> {
  await migrateLocalBubbles();
  await migrateJournalEntries();
  await refreshLocalBubbles();
  stopBubbleChanges = edenApi.subscribeBubbleChanges(() => {
    if (activeBubbleWrites === 0) void refreshLocalBubbles();
  });
}

async function readJournalMigration(): Promise<{
  entries: { entry: Entry; bubbles: BubbleTimelineNode[] }[];
  bubbles: BubbleTimelineNode[];
}> {
  const sourceEntries = await readJournalSourceEntries();
  const entries = sourceEntries
    .filter(isLegacyDatedJournalEntry)
    .sort(
      (left, right) => right.title.localeCompare(left.title) || right.created_at - left.created_at,
    );

  const loadedEntries: Entry[] = [];
  for (const entry of entries) {
    if (entry.content_loaded !== false || !window.api?.loadEntry) {
      loadedEntries.push(entry);
      continue;
    }
    loadedEntries.push((await window.api.loadEntry(entry.id)) ?? entry);
  }

  const migrationEntries = loadedEntries.map((entry) => ({
    entry,
    bubbles: createJournalBubblesFromEntry(entry),
  }));

  return {
    entries: migrationEntries,
    bubbles: migrationEntries.flatMap(({ bubbles }) => bubbles),
  };
}

async function migrateJournalEntries(): Promise<void> {
  if (journalCleanupRunning) return;
  journalCleanupRunning = true;
  try {
    const migration = await readJournalMigration();
    const deletable: string[] = [];
    for (const { entry, bubbles } of migration.entries) {
      if (bubbles.length === 0) continue;
      let complete = true;
      for (const bubble of bubbles) {
        try {
          await edenApi.migrateBubble(`dated-journal:${entry.id}`, bubble.id, bubble);
        } catch (err) {
          complete = false;
          console.warn("[eden] dated journal migration incomplete:", err);
        }
      }
      if (complete) deletable.push(entry.id);
    }
    await deleteImportedJournalEntries(deletable);
  } finally {
    journalCleanupRunning = false;
  }
}

async function readJournalSourceEntries(): Promise<Entry[]> {
  const entriesById = new Map((props.journalEntries ?? []).map((entry) => [entry.id, entry]));

  try {
    const allEntries = await window.api?.listAllEntries?.();
    for (const entry of allEntries ?? []) {
      entriesById.set(entry.id, entry);
    }
  } catch (err) {
    console.warn("[eden] legacy journal migration listAllEntries failed:", err);
  }

  return [...entriesById.values()];
}

async function deleteImportedJournalEntries(entryIds: string[]): Promise<void> {
  if (!window.api?.deleteEntry) return;
  let deletedCount = 0;
  for (const entryId of entryIds) {
    const result = await window.api.deleteEntry(entryId);
    if (result.ok) {
      deletedCount += 1;
      continue;
    }
    console.warn("[eden] legacy journal cleanup failed:", result);
  }
  if (deletedCount > 0) emit("journalMigrated");
}

function scheduleMidnightRefresh(): void {
  if (midnightTimer) clearTimeout(midnightTimer);
  const now = new Date();
  const nextMidnight = new Date(now.getFullYear(), now.getMonth(), now.getDate() + 1).getTime();
  midnightTimer = setTimeout(refreshLabels, Math.max(1, nextMidnight - now.getTime() + 25));
}

function refreshLabels(): void {
  labelNow.value = Date.now();
  scheduleMidnightRefresh();
}

function refreshLabelsWhenVisible(): void {
  if (document.visibilityState === "visible") refreshLabels();
}
</script>

<template>
  <section class="bubble-diary-view" data-testid="diary-view" aria-label="Дневник">
    <div class="bubble-diary-view__content">
      <form
        class="bubble-composer"
        aria-label="Новая мысль"
        @click="focusComposer"
        @submit.prevent="addDraftBubble"
      >
        <div class="bubble-composer__editor kosmos-scroll">
          <EditorContent
            v-if="composerEditor"
            :editor="composerEditor"
            class="bubble-composer__input"
          />
        </div>
        <div class="bubble-composer__footer">
          <button
            class="bubble-composer__submit"
            type="submit"
            :disabled="!canSubmitDraft"
            aria-label="Записать"
            title="Записать"
            data-testid="bubble-composer-submit"
          >
            <PhArrowUp aria-hidden="true" weight="bold" />
          </button>
        </div>
      </form>

      <section ref="timelineRef" class="bubble-timeline kosmos-scroll" aria-label="Лента дневника">
        <div
          v-if="localBubbles.length > 0"
          class="bubble-timeline__virtual-spacer"
          :style="{ height: virtualListHeight }"
        >
          <div
            v-for="{ row, node, threadRootId, continuesThread, replyTargetId } in virtualRows"
            :key="row.key"
            :ref="measureVirtualRow"
            class="bubble-timeline__virtual-row"
            :data-index="row.index"
            :data-thread-id="threadRootId"
            :style="{ transform: `translateY(${row.start}px)` }"
            @mouseenter="activateThread(threadRootId)"
            @mouseleave="deactivateThread(threadRootId, $event)"
            @focusin="activateThread(threadRootId)"
            @focusout="deactivateThread(threadRootId, $event)"
          >
            <BubbleTimelineItem
              :node="node"
              :now="labelNow"
              :continues-thread="continuesThread"
              :reply-target-id="replyTargetId"
              :thread-active="activeThreadId === threadRootId"
              @update="updateLocalBubble"
              @remove="deleteLocalBubble"
              @kind-change="updateBubbleKind"
              @reply="replyToBubble"
            />
          </div>
        </div>
        <div v-if="localBubbles.length === 0" class="bubble-timeline__empty">Пока нет записей</div>
      </section>
    </div>

    <BubbleDiaryCalendarSidebar
      v-if="props.calendarOpen"
      :bubbles="localBubbles"
      @date-select="scrollToDate"
    />
  </section>
</template>

<style scoped>
.bubble-diary-view {
  display: flex;
  min-height: 0;
  height: 100%;
  min-width: 0;
  flex-direction: row;
  overflow: hidden;
  color: var(--foreground);
}

.bubble-diary-view__content {
  position: relative;
  display: flex;
  width: 100%;
  min-height: 0;
  min-width: 0;
  flex: 1 1 auto;
  flex-direction: column;
  gap: 18px;
  overflow: hidden;
  padding: 10px 0 0;
}

.bubble-timeline {
  display: flex;
  width: 100%;
  min-width: 0;
  flex: 1 1 auto;
  flex-direction: column;
  gap: 0;
  overflow: auto;
  padding: 0 0 96px;
}

.bubble-timeline__empty {
  width: calc(100% - 32px);
  max-width: var(--kosmos-page-max-width, 700px);
  margin: 0 auto;
  border: 1px solid var(--border);
  border-radius: 8px;
  padding: 1.2rem;
  color: color-mix(in srgb, var(--muted-foreground) 92%, transparent);
  font-size: 0.9rem;
  text-align: center;
}

.bubble-timeline__virtual-spacer {
  position: relative;
  width: calc(100% - 32px);
  max-width: var(--kosmos-page-max-width, 700px);
  flex: 0 0 auto;
  margin: 0 auto;
}

.bubble-timeline__virtual-row {
  position: absolute;
  top: 0;
  left: 0;
  width: 100%;
  will-change: transform;
}

.bubble-composer {
  position: sticky;
  top: 0;
  z-index: 12;
  display: flex;
  flex: 0 0 auto;
  align-items: stretch;
  flex-direction: column;
  gap: 0;
  width: calc(100% - 32px);
  max-width: var(--kosmos-page-max-width, 700px);
  min-height: 114px;
  max-height: 80%;
  min-width: min(300px, 100%);
  margin: 0 auto;
  border: 1px solid color-mix(in srgb, var(--bg-app) 84%, white);
  border-radius: 8px;
  background: color-mix(in srgb, var(--bg-app) 92%, white);
  box-shadow:
    lch(0 0 0 / 0.04) 0 4px 4px -1px,
    lch(0 0 0 / 0.08) 0 1px 1px;
  box-sizing: border-box;
  color: lch(100 0 272);
  cursor: text;
  padding: 16px;
  font-family: var(--font-sans);
  font-size: 16px;
  font-weight: 400;
  line-height: 24px;
  text-rendering: optimizeLegibility;
  transition: border-color 0.15s ease-in-out;
  user-select: none;
}

.bubble-composer__editor {
  display: block;
  width: 100%;
  min-height: 48px;
  flex: 1 1 auto;
  overflow-y: auto;
  padding-bottom: 8px;
}

.bubble-composer__input {
  min-width: 0;
  overflow: visible;
  background: transparent;
  color: var(--foreground);
}

.bubble-composer__input :deep(.ProseMirror) {
  min-height: 48px;
  outline: none;
  background: transparent !important;
  padding: 0;
  caret-color: var(--accent);
  font-family: var(--font-sans);
  font-size: 16px;
  font-weight: 400;
  line-height: 24px;
  letter-spacing: 0;
  user-select: text;
  white-space: pre-wrap;
}

.bubble-composer__input :deep(.ProseMirror p) {
  margin: 0;
}

.bubble-composer__input :deep(.ProseMirror > *) {
  margin-top: 0;
  margin-bottom: 0.35rem;
}

.bubble-composer__input :deep(.ProseMirror > :last-child) {
  margin-bottom: 0;
}

.bubble-composer__input :deep(.ProseMirror p.is-editor-empty:first-child::before) {
  float: left;
  height: 0;
  color: color-mix(in srgb, var(--muted-foreground) 72%, transparent);
  content: attr(data-placeholder);
  pointer-events: none;
}

.bubble-composer__input :deep(.ProseMirror code) {
  border-radius: 4px;
  background: color-mix(in srgb, var(--foreground) 7%, transparent);
  padding: 0.05rem 0.25rem;
  font-family: var(--font-mono, ui-monospace, monospace);
  font-size: 0.9em;
}

.bubble-composer__input :deep(.ProseMirror pre) {
  overflow-x: auto;
  border-radius: 8px;
  background: color-mix(in srgb, var(--foreground) 6%, transparent);
  padding: 0.7rem 0.8rem;
  font-family: var(--font-mono, ui-monospace, monospace);
  font-size: 0.86rem;
  line-height: 1.55;
}

.bubble-composer__footer {
  display: flex;
  width: 100%;
  height: 32px;
  flex: 0 0 32px;
  align-items: center;
  justify-content: flex-end;
  margin: 0;
}

.bubble-composer__submit {
  display: inline-flex;
  width: 32px;
  min-width: 32px;
  height: 32px;
  align-items: center;
  justify-content: center;
  border: 0;
  border-radius: var(--radius-pill, 999px);
  background: #fff;
  color: var(--background);
  padding: 0;
  cursor: default;
  user-select: none;
  transition: transform 120ms ease;
}

.bubble-composer__submit svg {
  width: 17px;
  height: 17px;
}

.bubble-composer__submit:hover:not(:disabled) {
  background: #fff;
}

.bubble-composer__submit:active:not(:disabled) {
  transform: scale(0.96);
}

.bubble-composer__submit:disabled {
  background: color-mix(in srgb, var(--foreground) 16%, transparent);
  color: color-mix(in srgb, var(--foreground) 42%, transparent);
}

@media (max-width: 520px) {
  .bubble-composer {
    width: 100%;
  }

  .bubble-timeline {
    width: 100%;
  }
}
</style>
