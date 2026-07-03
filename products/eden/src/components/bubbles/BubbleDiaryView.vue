<script setup lang="ts">
import { computed, nextTick, onMounted, ref, watch } from "vue";
import type { JSONContent } from "@tiptap/core";
import Placeholder from "@tiptap/extension-placeholder";
import StarterKit from "@tiptap/starter-kit";
import { EditorContent, useEditor } from "@tiptap/vue-3";
import BubbleDiaryCalendarSidebar from "./BubbleDiaryCalendarSidebar.vue";
import BubbleTimelineItem from "./BubbleTimelineItem.vue";
import {
  LOCAL_BUBBLES_STORAGE_KEY,
  LOCAL_BUBBLES_STORAGE_VERSION,
  createDraftBubble,
  createJournalBubblesFromEntry,
  decodeLocalBubblesStorage,
  encodeLocalBubblesStorage,
  isLegacyDatedJournalEntry,
  normalizeLocalBubbles,
  parseBubbleDraft,
  plainTextToTiptapDoc,
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
const journalImported = ref(false);
const contentRef = ref<HTMLElement | null>(null);
const composerHeightPx = ref(78);
let journalCleanupRunning = false;

const draftPreview = computed(() => parseBubbleDraft(draftPlainText.value));
const canSubmitDraft = computed(() => draftPreview.value.text.length > 0);

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
    requestAnimationFrame(syncComposerHeight);
  },
});

onMounted(() => {
  void refreshLocalBubbles();
  void nextTick(syncComposerHeight);
});

watch(localBubbles, () => persistLocalBubbles(), { deep: true });

watch(
  () => props.journalEntries,
  () => {
    void refreshLocalBubbles();
  },
);

function addDraftBubble(): void {
  const editor = composerEditor.value;
  if (!editor) return;

  const bubble = createDraftBubble(editor.getJSON() as JSONContent, new Date(), editor.getText());
  if (!bubble) return;

  localBubbles.value = [bubble, ...localBubbles.value];
  editor.commands.clearContent();
  draftPlainText.value = "";
  requestAnimationFrame(syncComposerHeight);
}

function focusComposer(event: MouseEvent): void {
  const target = event.target;
  if (target instanceof Element && target.closest("button")) return;
  composerEditor.value?.commands.focus("end");
}

function syncComposerHeight(): void {
  const editorElement = composerEditor.value?.view.dom;
  if (!editorElement) return;
  composerHeightPx.value = Math.max(78, Math.ceil(editorElement.scrollHeight + 22));
}

function updateLocalBubble(id: string, input: string): void {
  const draft = parseBubbleDraft(input);
  if (!draft.text) return;

  localBubbles.value = localBubbles.value.map((bubble) =>
    bubble.id === id
      ? {
          ...bubble,
          text: draft.text,
          contentJson: plainTextToTiptapDoc(draft.text),
          tags: draft.tags,
        }
      : bubble,
  );
}

function deleteLocalBubble(id: string): void {
  localBubbles.value = localBubbles.value.filter((bubble) => bubble.id !== id);
}

function updateBubbleKind(id: string, kind: BubbleKind): void {
  localBubbles.value = localBubbles.value.map((bubble) =>
    bubble.id === id ? { ...bubble, kind } : bubble,
  );
}

function scrollToDate(date: string): void {
  const target = [
    ...(contentRef.value?.querySelectorAll<HTMLElement>("[data-bubble-date]") ?? []),
  ].find((element) => element.dataset.bubbleDate === date);
  target?.scrollIntoView({ block: "center", behavior: "smooth" });
}

function readLocalBubbles(): BubbleTimelineNode[] {
  return readLocalBubbleState().bubbles;
}

function persistLocalBubbles(): void {
  localStorage.setItem(
    LOCAL_BUBBLES_STORAGE_KEY,
    encodeLocalBubblesStorage(localBubbles.value, {
      journalImported: journalImported.value,
    }),
  );
}

function readLocalBubbleState(): {
  bubbles: BubbleTimelineNode[];
  journalImported: boolean;
} {
  try {
    const raw = JSON.parse(localStorage.getItem(LOCAL_BUBBLES_STORAGE_KEY) ?? "null");
    const record = raw && typeof raw === "object" ? (raw as Record<string, unknown>) : null;
    return {
      bubbles: decodeLocalBubblesStorage(raw),
      journalImported:
        record?.version === LOCAL_BUBBLES_STORAGE_VERSION && record.journalImported === true,
    };
  } catch {
    return { bubbles: [], journalImported: false };
  }
}

async function refreshLocalBubbles(): Promise<void> {
  const state = readLocalBubbleState();
  const migration = await readJournalMigration();
  const journalBubbles = state.journalImported ? [] : migration.bubbles;
  const nextBubbles = mergeBubbles(state.bubbles, journalBubbles);

  journalImported.value = state.journalImported;
  localBubbles.value = nextBubbles;

  const deletableEntryIds = migration.entries
    .filter(
      ({ bubbles }) =>
        bubbles.length > 0 && bubbles.every((bubble) => hasBubble(nextBubbles, bubble.id)),
    )
    .map(({ entry }) => entry.id);
  if (deletableEntryIds.length > 0) {
    await deleteImportedJournalEntries(deletableEntryIds);
  }
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

function mergeBubbles(
  local: BubbleTimelineNode[],
  journal: BubbleTimelineNode[],
): BubbleTimelineNode[] {
  const byId = new Map<string, BubbleTimelineNode>();
  for (const bubble of normalizeLocalBubbles([...local, ...journal])) {
    if (!byId.has(bubble.id)) byId.set(bubble.id, bubble);
  }
  return [...byId.values()];
}

function hasBubble(bubbles: BubbleTimelineNode[], id: string): boolean {
  return bubbles.some((bubble) => bubble.id === id);
}

async function deleteImportedJournalEntries(entryIds: string[]): Promise<void> {
  if (journalCleanupRunning || !window.api?.deleteEntry) return;
  journalCleanupRunning = true;

  let deletedCount = 0;
  try {
    for (const entryId of entryIds) {
      const result = await window.api.deleteEntry(entryId);
      if (result.ok) {
        deletedCount += 1;
        continue;
      }
      console.warn("[eden] legacy journal cleanup failed:", result);
    }

    if (deletedCount === entryIds.length) {
      journalImported.value = true;
      persistLocalBubbles();
    }

    if (deletedCount > 0) emit("journalMigrated");
  } finally {
    journalCleanupRunning = false;
  }
}
</script>

<template>
  <section class="bubble-diary-view" data-testid="diary-view" aria-label="Дневник">
    <div ref="contentRef" class="bubble-diary-view__content kosmos-scroll">
      <form
        class="bubble-composer"
        :style="{ minHeight: `${composerHeightPx}px` }"
        aria-label="Новая мысль"
        @click="focusComposer"
        @submit.prevent="addDraftBubble"
      >
        <div class="bubble-composer__editor">
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
            data-testid="bubble-composer-submit"
          >
            Записать
          </button>
        </div>
      </form>

      <section class="bubble-timeline" aria-label="Лента дневника">
        <BubbleTimelineItem
          v-for="node in localBubbles"
          :key="node.id"
          :node="node"
          @update="updateLocalBubble"
          @remove="deleteLocalBubble"
          @kind-change="updateBubbleKind"
        />
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
  background: transparent;
  color: var(--foreground);
}

.bubble-diary-view__content {
  position: relative;
  display: flex;
  min-height: 0;
  min-width: 0;
  flex: 1 1 auto;
  flex-direction: column;
  overflow: auto;
}

.bubble-timeline {
  display: flex;
  width: min(880px, calc(100% - 2rem));
  min-width: 0;
  flex: 1 0 auto;
  flex-direction: column;
  gap: 0;
  margin: 0 auto;
  padding: 18px 0 96px;
}

.bubble-timeline__empty {
  border: 1px solid var(--border);
  border-radius: 8px;
  padding: 1.2rem;
  color: color-mix(in srgb, var(--muted-foreground) 92%, transparent);
  font-size: 0.9rem;
  text-align: center;
}

.bubble-composer {
  position: sticky;
  top: 0;
  z-index: 12;
  display: grid;
  grid-template-columns: minmax(0, 1fr) auto;
  align-items: end;
  gap: 12px;
  width: min(880px, calc(100% - 2rem));
  min-height: 55px;
  margin: 10px auto 0;
  border: 1px solid var(--border-color-strong);
  border-radius: var(--radius-lg, 12px);
  background: var(--bg-elevated);
  cursor: text;
  padding: 14px 8px 8px 12px;
}

.bubble-composer__editor {
  display: block;
  width: 100%;
  min-height: 48px;
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
  font-size: 1rem;
  line-height: 1.45;
  letter-spacing: 0;
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
  align-items: center;
  justify-content: flex-end;
  margin: 0;
}

.bubble-composer__submit {
  display: inline-flex;
  min-width: 4.1rem;
  height: 28px;
  align-items: center;
  justify-content: center;
  border: 0;
  border-radius: 4px 4px 8px;
  background: var(--accent);
  color: var(--accent-foreground, var(--background));
  padding: 0 18px;
  font-size: 0.84rem;
  font-weight: 650;
  line-height: 28px;
  cursor: default;
  user-select: none;
  transition:
    background-color 120ms ease,
    opacity 120ms ease,
    transform 120ms ease;
}

.bubble-composer__submit:hover:not(:disabled) {
  background: color-mix(in srgb, var(--accent) 88%, var(--foreground));
}

.bubble-composer__submit:active:not(:disabled) {
  transform: scale(0.96);
}

.bubble-composer__submit:disabled {
  opacity: 0.45;
}

@media (max-width: 520px) {
  .bubble-composer {
    width: calc(100% - 1.5rem);
  }

  .bubble-timeline {
    width: calc(100% - 1.5rem);
    padding-top: 14px;
  }
}
</style>
