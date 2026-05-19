<template>
  <div :class="['editor-wrapper', 'kosmos-scroll', { 'focus-mode': zenMode }]">
    <div class="editor-header">
      <div class="editor-rail editor-header-rail">
        <div class="editor-header-main">
          <input
            ref="titleInput"
            v-model="title"
            class="title-input"
            :placeholder="UNTITLED_ENTRY_PLACEHOLDER"
            :readonly="isJournalEntry"
            :tabindex="isJournalEntry ? -1 : 0"
          />
          <div ref="noteTypeMenu" class="note-type-inline">
            <button
              class="note-type-trigger"
              data-testid="typed-note-trigger"
              type="button"
              :style="{ '--note-type-accent': activeNoteType?.color ?? 'var(--text-tertiary)' }"
              @click="toggleNoteTypeMenu"
            >
              {{ activeNoteType?.name ?? "Заметка" }}
            </button>
            <div v-if="isNoteTypeMenuOpen" class="note-type-menu" data-testid="typed-note-menu">
              <button
                class="note-type-menu-item"
                type="button"
                @mouseenter="isTypePickerOpen = false"
                @click="openActiveTypeSettings"
              >
                Открыть объект
              </button>
              <div class="note-type-menu-divider" aria-hidden="true"></div>
              <div
                class="note-type-menu-submenu"
                @mouseenter="isTypePickerOpen = true"
              >
                <button
                  class="note-type-menu-item note-type-menu-item--submenu"
                  type="button"
                  @click="toggleTypePicker"
                >
                  <span>Изменить тип</span>
                  <span class="note-type-menu-chevron" aria-hidden="true">›</span>
                </button>
                <div v-if="isTypePickerOpen" class="note-type-submenu">
                  <button
                    v-for="noteType in typePickerOptions"
                    :key="noteType.id"
                    :class="['note-type-menu-item', 'note-type-submenu-item', noteType.id === noteTypeId && 'is-active']"
                    type="button"
                    @click="handleNoteTypeChange(noteType.id)"
                  >
                    <span
                      class="note-type-submenu-item__swatch"
                      :style="{
                        '--note-type-item-color': noteType.color ?? 'var(--text-secondary)',
                        '--note-type-item-icon-src': `url(${getNoteTypeIconSrc(noteType)})`,
                      }"
                    >
                      <span
                        v-if="getNoteTypeIconSrc(noteType)"
                        class="note-type-submenu-item__icon"
                        aria-hidden="true"
                      ></span>
                      <span v-else class="note-type-submenu-item__dot"></span>
                    </span>
                    <span class="note-type-submenu-item__label">{{ noteType.name }}</span>
                  </button>
                </div>
              </div>
            </div>
          </div>
          <TypedHeader
            :active-note-type="activeNoteType"
            :title="title"
            :header-props="headerProps"
            :validation-error="headerValidationError"
            :all-entries="allEntries"
            :current-entry-id="entry.id"
            :show-type-row="false"
            @header-prop-change="handleHeaderPropChange"
            @relation-navigate="props.onNavigate"
          />
        </div>
        <div class="editor-header-actions">
          <span v-if="saveConflict" class="save-conflict-badge">{{ saveConflict }}</span>
        </div>
      </div>
    </div>
    <div class="editor-content-area" @click="editor?.commands.focus()">
      <div class="editor-rail editor-content-rail">
        <EditorContent :editor="editor ?? null" />
      </div>
    </div>
  </div>
</template>

<script setup lang="ts">
import { computed, nextTick, onBeforeUnmount, ref, useTemplateRef, watch, watchEffect } from "vue";
import { useEditor, EditorContent, VueRenderer, VueNodeViewRenderer } from "@tiptap/vue-3";
import StarterKit from "@tiptap/starter-kit";
import { Markdown } from "@tiptap/markdown";
import Placeholder from "@tiptap/extension-placeholder";
import CodeBlockLowlight from "@tiptap/extension-code-block-lowlight";
import { all, createLowlight } from "lowlight";
import Typography from "@tiptap/extension-typography";
import type { Editor as TiptapEditor, Range } from "@tiptap/vue-3";
import { Wikilink } from "./Wikilink";
// InlineCaret удалён — widget-decoration на каждом cursor position ломал
// drag-selection (mousedown на widget → ProseMirror не разрешал mouse
// position в text offset, drag только перемещал каретку). Native browser
// caret и так работает корректно.
import WikilinkList from "./WikilinkList.vue";
import { SlashCommand } from "./SlashCommand";
import SlashCommandList from "./SlashCommandList.vue";
import TypedHeader from "@/components/typed-notes/TypedHeader.vue";
import CodeBlockView from "@/components/CodeBlockView.vue";
import {
  createDefaultHeaderProps,
  resolveNoteTypeHeaderLayout,
  safeParseHeaderProps,
  validateHeaderProps,
} from "@/lib/typedNotes";
import { SYSTEM_TYPE_NOTE_ID, SYSTEM_TYPE_JOURNAL_ID } from "@/lib/systemTypes";
import { countCharsInProseMirrorDoc, countCharsInProseMirrorNode } from "@/lib/charCount";
import { objectIconUri } from "@/lib/iconResolver";
import {
  getEditableEntryTitle,
  resolveStoredEntryTitle,
  syncUntitledEntryTitleFlag,
  UNTITLED_ENTRY_PLACEHOLDER,
} from "@/lib/entryTitles";
import "./Editor.css";

const DEFAULT_DOCUMENT = {
  type: "doc",
  content: [{ type: "paragraph" }],
} as const;

const DEFAULT_DOCUMENT_JSON = JSON.stringify(DEFAULT_DOCUMENT);

const AUTOSAVE_DEBOUNCE_MS = 800;
const PERF_SAMPLE_LIMIT = 120;
const TRACKED_EDIT_KEYS = new Set(["Backspace", "Delete", "Enter", "Tab"]);

type PerfMetricKey = "inputToNextPaint" | "updateToNextPaint" | "saveDuration";

type EditorPerfTracker = EdenPerfTracker & {
  recordMetric: (metric: PerfMetricKey, durationMs: number) => void;
  recordLongTask: (durationMs: number) => void;
};

const props = defineProps<{
  entry: Entry;
  allEntries: Entry[];
  noteTypes: NoteType[];
  zenMode?: boolean;
  onSave: (entry: Entry) => Promise<SaveEntryResult | null>;
  onNavigate: (entryId: string) => void;
  onOpenTypeSettings: (noteTypeId: string) => void;
}>();

const emit = defineEmits<{
  exitZen: [];
  liveCharCount: [count: number];
}>();

function emitLiveCharCount() {
  if (!editor.value) {
    emit("liveCharCount", countCharsInProseMirrorDoc(lastPersistedContentJson) ?? 0);
    return;
  }
  emit("liveCharCount", countCharsInProseMirrorNode(editor.value.getJSON()));
}

const title = ref(getEditableEntryTitle(props.entry.title, props.entry.header_props_json));
const noteTypeId = ref<string>(props.entry.type_id ?? SYSTEM_TYPE_NOTE_ID);
const headerLayout = ref<string | null>(props.entry.header_layout);
const headerProps = ref<Record<string, unknown>>(
  safeParseHeaderProps(
    props.noteTypes.find((noteType) => noteType.id === (props.entry.type_id ?? SYSTEM_TYPE_NOTE_ID)) ?? null,
    props.entry.header_props_json,
  ),
);
const saveConflict = ref<string | null>(null);
const headerValidationError = ref<string | null>(null);
const isNoteTypeMenuOpen = ref(false);
const isTypePickerOpen = ref(false);
const noteTypeMenuRef = useTemplateRef<HTMLDivElement>("noteTypeMenu");
const titleInputRef = useTemplateRef<HTMLInputElement>("titleInput");

let saveRunId = 0;
let documentRevision = 0;
let savedDocumentRevision = 0;
let metadataRevision = 0;
let savedMetadataRevision = 0;
let isHydrating = true;
let autosaveTimer: number | null = null;
let reconcileTimer: number | null = null;
let lastPersistedContentJson = normalizeContentJson(props.entry.content_json);
let lastPersistedMarkdown = "";
let lastPersistedTitle = props.entry.title;
let lastPersistedNoteTypeId = props.entry.type_id ?? SYSTEM_TYPE_NOTE_ID;
let lastPersistedHeaderLayout = props.entry.header_layout ?? "default";
let lastPersistedHeaderPropsJson = normalizeHeaderPropsJson(props.entry.header_props_json);

const perfTracker = createPerfTracker();

const allEntriesHolder = { current: props.allEntries };
watch(
  () => props.allEntries,
  (value) => {
    allEntriesHolder.current = value;
  },
  { immediate: true },
);

const activeNoteType = computed(
  () => props.noteTypes.find((noteType) => noteType.id === noteTypeId.value) ?? null,
);

// Дневник: title заметки — это ISO-дата (`YYYY-MM-DD`), её менять нельзя
// (lookup в openTodayJournal ищет именно по точному совпадению с сегодня).
// readonly + tabindex=-1 защищает от случайного ввода (например слайдер фокуса
// в zen-режиме, попадание `/` в title через keyboard navigation).
const isJournalEntry = computed(() => noteTypeId.value === SYSTEM_TYPE_JOURNAL_ID);

const typePickerOptions = computed(() => props.noteTypes.filter((noteType) => Boolean(noteType.id)));

function getNoteTypeIconSrc(noteType: NoteType | null) {
  return noteType?.icon ? objectIconUri(noteType.icon) : "";
}

const lowlight = createLowlight(all);

const EdenCodeBlock = CodeBlockLowlight.extend({
  addAttributes() {
    return {
      ...this.parent?.(),
      wrap: {
        default: true,
        parseHTML: (element: HTMLElement) => element.getAttribute("data-wrap") !== "false",
        renderHTML: (attributes: { wrap?: boolean }) => ({
          "data-wrap": attributes.wrap === false ? "false" : "true",
        }),
      },
    };
  },
  addNodeView() {
    return VueNodeViewRenderer(CodeBlockView);
  },
});

const extensions = [
  StarterKit.configure({ codeBlock: false }),
  Markdown,
  Placeholder.configure({ placeholder: "Начни писать что-нибудь интересное..." }),
  EdenCodeBlock.configure({
    lowlight,
    enableTabIndentation: true,
    tabSize: 4,
    defaultLanguage: null,
  }),
  Typography,
  Wikilink.configure({
    suggestion: {
      items: ({ query }: { query: string }) =>
        allEntriesHolder.current
          .filter((item) => item.title.toLowerCase().includes(query.toLowerCase()))
          .slice(0, 10),
      render: () => {
        let component: InstanceType<typeof VueRenderer>;
        let popup: SuggestionPopupHandle | null = null;

        return {
          onStart: (renderProps: any) => {
            component = new VueRenderer(WikilinkList, {
              props: renderProps,
              editor: renderProps.editor,
            });
            popup = createSuggestionPopup(
              component.element as HTMLElement,
              getSuggestionClientRect(renderProps),
            );
          },
          onUpdate(renderProps: any) {
            component.updateProps(renderProps);
            popup?.update(getSuggestionClientRect(renderProps));
          },
          onKeyDown(renderProps: any) {
            if (renderProps.event.key === "Escape") {
              popup?.destroy();
              return true;
            }
            return (component.ref as any)?.onKeyDown(renderProps) ?? false;
          },
          onExit() {
            popup?.destroy();
            popup = null;
            component.destroy();
          },
        };
      },
    },
  }),
  SlashCommand.configure({
    suggestion: {
      items: ({ query }: { query: string }) =>
        [
          {
            title: "Заголовок",
            icon: "H1",
            command: ({ editor, range }: { editor: TiptapEditor; range: Range }) =>
              editor.chain().focus().deleteRange(range).setNode("heading", { level: 1 }).run(),
          },
          {
            title: "Подзаголовок",
            icon: "H2",
            command: ({ editor, range }: { editor: TiptapEditor; range: Range }) =>
              editor.chain().focus().deleteRange(range).setNode("heading", { level: 2 }).run(),
          },
          {
            title: "Текст",
            icon: "P",
            command: ({ editor, range }: { editor: TiptapEditor; range: Range }) =>
              editor.chain().focus().deleteRange(range).setNode("paragraph").run(),
          },
          {
            title: "Список",
            icon: "•",
            command: ({ editor, range }: { editor: TiptapEditor; range: Range }) =>
              editor.chain().focus().deleteRange(range).toggleBulletList().run(),
          },
          {
            title: "Код",
            icon: "{}",
            command: ({ editor, range }: { editor: TiptapEditor; range: Range }) =>
              editor.chain().focus().deleteRange(range).toggleCodeBlock().run(),
          },
          {
            title: "Цитата",
            icon: '"',
            command: ({ editor, range }: { editor: TiptapEditor; range: Range }) =>
              editor.chain().focus().deleteRange(range).toggleBlockquote().run(),
          },
        ].filter((item) => item.title.toLowerCase().includes(query.toLowerCase())),
      render: () => {
        let component: InstanceType<typeof VueRenderer>;
        let popup: SuggestionPopupHandle | null = null;

        return {
          onStart: (renderProps: any) => {
            component = new VueRenderer(SlashCommandList, {
              props: renderProps,
              editor: renderProps.editor,
            });
            popup = createSuggestionPopup(
              component.element as HTMLElement,
              getSuggestionClientRect(renderProps),
            );
          },
          onUpdate(renderProps: any) {
            component.updateProps(renderProps);
            popup?.update(getSuggestionClientRect(renderProps));
          },
          onKeyDown(renderProps: any) {
            if (renderProps.event.key === "Escape") {
              popup?.destroy();
              return true;
            }
            return (component.ref as any)?.onKeyDown(renderProps) ?? false;
          },
          onExit() {
            popup?.destroy();
            popup = null;
            component.destroy();
          },
        };
      },
    },
  }),
];

const editor = useEditor({
  extensions,
  content: parseContentJson(lastPersistedContentJson),
  editable: true,
  onUpdate: () => {
    const startedAt = performance.now();
    markDocumentDirty();
    emitLiveCharCount();
    requestAnimationFrame(() => {
      perfTracker.recordMetric("updateToNextPaint", performance.now() - startedAt);
    });
  },
});

function createPerfTracker(): EditorPerfTracker {
  const metrics: Record<PerfMetricKey, number[]> = {
    inputToNextPaint: [],
    updateToNextPaint: [],
    saveDuration: [],
  };

  let longTaskCount = 0;
  let longTaskMaxDurationMs = 0;

  return {
    recordMetric(metric, durationMs) {
      if (!Number.isFinite(durationMs) || durationMs < 0) return;
      const samples = metrics[metric];
      samples.push(durationMs);
      if (samples.length > PERF_SAMPLE_LIMIT) {
        samples.splice(0, samples.length - PERF_SAMPLE_LIMIT);
      }
    },
    recordLongTask(durationMs) {
      if (!Number.isFinite(durationMs) || durationMs < 0) return;
      longTaskCount += 1;
      longTaskMaxDurationMs = Math.max(longTaskMaxDurationMs, durationMs);
    },
    reset() {
      for (const key of Object.keys(metrics) as PerfMetricKey[]) {
        metrics[key] = [];
      }
      longTaskCount = 0;
      longTaskMaxDurationMs = 0;
    },
    getSummary() {
      return {
        inputToNextPaint: summarizeMetric(metrics.inputToNextPaint),
        updateToNextPaint: summarizeMetric(metrics.updateToNextPaint),
        saveDuration: summarizeMetric(metrics.saveDuration),
        longTaskCount,
        longTaskMaxDurationMs,
      };
    },
  };
}

function summarizeMetric(samples: number[]): EdenPerfMetricSummary {
  if (samples.length === 0) {
    return {
      count: 0,
      minMs: 0,
      maxMs: 0,
      avgMs: 0,
      p50Ms: 0,
      p95Ms: 0,
      p99Ms: 0,
    };
  }

  const sorted = [...samples].sort((left, right) => left - right);
  const total = sorted.reduce((sum, value) => sum + value, 0);

  return {
    count: sorted.length,
    minMs: roundMetric(sorted[0] ?? 0),
    maxMs: roundMetric(sorted[sorted.length - 1] ?? 0),
    avgMs: roundMetric(total / sorted.length),
    p50Ms: percentile(sorted, 0.5),
    p95Ms: percentile(sorted, 0.95),
    p99Ms: percentile(sorted, 0.99),
  };
}

function percentile(sorted: number[], quantile: number) {
  if (sorted.length === 0) return 0;
  const index = Math.min(sorted.length - 1, Math.max(0, Math.ceil(sorted.length * quantile) - 1));
  return roundMetric(sorted[index] ?? 0);
}

function roundMetric(value: number) {
  return Math.round(value * 100) / 100;
}

function normalizeContentJson(contentJson: string | null | undefined) {
  return contentJson && contentJson.trim() ? contentJson : DEFAULT_DOCUMENT_JSON;
}

function parseContentJson(contentJson: string) {
  try {
    return JSON.parse(contentJson) as Record<string, unknown>;
  } catch {
    return DEFAULT_DOCUMENT;
  }
}

function getSerializedEditorContent() {
  if (!editor.value) {
    return lastPersistedContentJson;
  }

  return JSON.stringify(editor.value.getJSON());
}

function getSerializedEditorMarkdown() {
  return editor.value?.storage?.markdown?.getMarkdown?.() ?? "";
}

function getSuggestionClientRect(renderProps: {
  clientRect?: (() => DOMRect | null) | null;
  editor: TiptapEditor;
  range: Range;
}) {
  if (renderProps.clientRect) {
    return renderProps.clientRect;
  }

  return () => {
    const { left, right, top, bottom } = renderProps.editor.view.coordsAtPos(renderProps.range.from);
    return new DOMRect(left, top, Math.max(1, right - left), Math.max(1, bottom - top));
  };
}

interface SuggestionPopupHandle {
  destroy: () => void;
  update: (clientRectGetter: () => DOMRect | null) => void;
}

function createSuggestionPopup(
  element: HTMLElement,
  clientRectGetter: () => DOMRect | null,
): SuggestionPopupHandle {
  const container = document.createElement("div");
  container.style.position = "fixed";
  container.style.left = "0";
  container.style.top = "0";
  container.style.zIndex = "1000";
  container.style.pointerEvents = "auto";
  document.body.appendChild(container);
  container.appendChild(element);

  const applyPosition = (nextClientRectGetter: () => DOMRect | null) => {
    const rect = nextClientRectGetter();
    if (!rect) return;
    container.style.left = `${Math.round(rect.left)}px`;
    container.style.top = `${Math.round(rect.bottom + 6)}px`;
  };

  applyPosition(clientRectGetter);

  return {
    update(nextClientRectGetter) {
      applyPosition(nextClientRectGetter);
    },
    destroy() {
      container.remove();
    },
  };
}

function normalizeHeaderPropsJson(headerPropsJson: string | null | undefined) {
  if (!headerPropsJson?.trim()) {
    return JSON.stringify({});
  }

  try {
    return JSON.stringify(JSON.parse(headerPropsJson));
  } catch {
    return JSON.stringify({});
  }
}

function isDirty() {
  return documentRevision !== savedDocumentRevision || metadataRevision !== savedMetadataRevision;
}

function resetRevisionBaseline() {
  documentRevision = 0;
  savedDocumentRevision = 0;
  metadataRevision = 0;
  savedMetadataRevision = 0;
}

function markDocumentDirty() {
  if (isHydrating) return;
  documentRevision += 1;
  saveConflict.value = null;
  schedulePersistedStateReconciliation();
  scheduleAutoSave();
}

function markMetadataDirty() {
  if (isHydrating) return;
  metadataRevision += 1;
  saveConflict.value = null;
  schedulePersistedStateReconciliation();
  scheduleAutoSave();
}

function schedulePersistedStateReconciliation() {
  if (reconcileTimer !== null) {
    window.clearTimeout(reconcileTimer);
  }

  reconcileTimer = window.setTimeout(() => {
    reconcileTimer = null;

    if (!editor.value) return;

    const normalizedTitle = resolveStoredEntryTitle(
      title.value,
      lastPersistedTitle,
      lastPersistedHeaderPropsJson,
    );
    const normalizedHeaderPropsJson = normalizeHeaderPropsJson(
      JSON.stringify(
        syncUntitledEntryTitleFlag(
          headerProps.value,
          title.value,
          lastPersistedTitle,
          lastPersistedHeaderPropsJson,
        ),
      ),
    );
    const contentJson = getSerializedEditorContent();
    const markdown = getSerializedEditorMarkdown();

    if (
      matchesPersistedState({
        title: normalizedTitle,
        noteTypeId: noteTypeId.value,
        headerLayout: headerLayout.value,
        headerPropsJson: normalizedHeaderPropsJson,
        contentJson,
        markdown,
      })
    ) {
      savedDocumentRevision = documentRevision;
      savedMetadataRevision = metadataRevision;
    }
  }, 180);
}

function scheduleAutoSave() {
  if (autosaveTimer !== null) {
    window.clearTimeout(autosaveTimer);
  }

  autosaveTimer = window.setTimeout(() => {
    autosaveTimer = null;
    void save();
  }, AUTOSAVE_DEBOUNCE_MS);
}

function clearScheduledWork() {
  if (autosaveTimer !== null) {
    window.clearTimeout(autosaveTimer);
    autosaveTimer = null;
  }

  if (reconcileTimer !== null) {
    window.clearTimeout(reconcileTimer);
    reconcileTimer = null;
  }
}

function updatePersistedMetadataBaseline(options: {
  title: string;
  noteTypeId: string | null;
  headerLayout: string | null;
  headerPropsJson: string;
}) {
  lastPersistedTitle = options.title;
  lastPersistedNoteTypeId = options.noteTypeId;
  lastPersistedHeaderLayout = options.headerLayout;
  lastPersistedHeaderPropsJson = options.headerPropsJson;
}

function matchesPersistedState(options: {
  title: string;
  noteTypeId: string | null;
  headerLayout: string | null;
  headerPropsJson: string;
  contentJson: string;
  markdown: string;
}) {
  const metadataMatches =
    options.title === lastPersistedTitle &&
    options.noteTypeId === lastPersistedNoteTypeId &&
    options.headerLayout === lastPersistedHeaderLayout &&
    options.headerPropsJson === lastPersistedHeaderPropsJson;

  return (
    metadataMatches &&
    (options.contentJson === lastPersistedContentJson || options.markdown === lastPersistedMarkdown)
  );
}

function hydrateFromEntry(entry: Entry) {
  const nextTypeId = entry.type_id ?? SYSTEM_TYPE_NOTE_ID;
  const nextNoteType = props.noteTypes.find((noteType) => noteType.id === nextTypeId) ?? null;
  const nextContentJson = normalizeContentJson(entry.content_json);

  isHydrating = true;
  title.value = getEditableEntryTitle(entry.title, entry.header_props_json);
  noteTypeId.value = nextTypeId;
  headerLayout.value = entry.header_layout ?? "default";
  headerProps.value = safeParseHeaderProps(nextNoteType, entry.header_props_json);
  headerValidationError.value = null;
  saveConflict.value = null;
  lastPersistedContentJson = nextContentJson;
  updatePersistedMetadataBaseline({
    title: entry.title,
    noteTypeId: nextTypeId,
    headerLayout: entry.header_layout ?? "default",
    headerPropsJson: normalizeHeaderPropsJson(entry.header_props_json),
  });
  resetRevisionBaseline();
  clearScheduledWork();

  if (editor.value) {
    editor.value.commands.setContent(parseContentJson(nextContentJson), false);
  }

  queueMicrotask(() => {
    lastPersistedContentJson = getSerializedEditorContent();
    lastPersistedMarkdown = getSerializedEditorMarkdown();
    isHydrating = false;
    emitLiveCharCount();
  });
}

async function save() {
  if (!editor.value || !isDirty()) return;

  const headerValidation = validateHeaderProps(activeNoteType.value, headerProps.value);
  if (!headerValidation.success) {
    headerValidationError.value = "Проверьте поля верхней части заметки";
    return;
  }

  headerValidationError.value = null;

  const currentSaveRunId = ++saveRunId;
  const saveStartedAt = performance.now();

  const normalizedTitle = resolveStoredEntryTitle(
    title.value,
    lastPersistedTitle,
    lastPersistedHeaderPropsJson,
  );
  const normalizedHeaderPropsJson = JSON.stringify(
    syncUntitledEntryTitleFlag(
      headerValidation.data,
      title.value,
      lastPersistedTitle,
      lastPersistedHeaderPropsJson,
    ),
  );
  const content_json = getSerializedEditorContent();
  const markdown = getSerializedEditorMarkdown();

  if (
    matchesPersistedState({
      title: normalizedTitle,
      noteTypeId: noteTypeId.value,
      headerLayout: headerLayout.value,
      headerPropsJson: normalizedHeaderPropsJson,
      contentJson: content_json,
      markdown,
    })
  ) {
    savedDocumentRevision = documentRevision;
    savedMetadataRevision = metadataRevision;
    return;
  }

  // Throw'ы из onSave (network error, ARK недоступен, преload сбойнул) до
  // 2026-05-18 пропускались наружу как unhandled rejection — autosave таймер
  // молча ретраил один и тот же контент каждые 800ms бесконечно, lastPersisted*
  // не обновлялся, пользователь не видел никаких индикаторов. Surface через
  // saveConflict; autosave всё равно повторит на следующую правку.
  let saveResult: Awaited<ReturnType<typeof props.onSave>>;
  try {
    saveResult = await props.onSave({
      ...props.entry,
      title: normalizedTitle,
      content_json,
      type_id: noteTypeId.value,
      header_layout: headerLayout.value,
      header_props_json: normalizedHeaderPropsJson,
      schema_version: 1,
      updated_at: Date.now(),
    });
  } catch (e) {
    if (currentSaveRunId !== saveRunId) return;
    console.error("[eden] save failed:", e);
    saveConflict.value = "Не удалось сохранить — нет связи с ARK";
    return;
  }

  if (!saveResult) return;
  if (currentSaveRunId !== saveRunId) return;

  if (!saveResult.ok && saveResult.reason === "duplicate_title") {
    saveConflict.value = "Заметка с таким названием уже есть в этой папке";
    return;
  }

  if (!saveResult.ok && saveResult.reason === "invalid_type_metadata") {
    headerValidationError.value = saveResult.message;
    return;
  }

  saveConflict.value = null;
  lastPersistedContentJson = getSerializedEditorContent();
  lastPersistedMarkdown = getSerializedEditorMarkdown();
  updatePersistedMetadataBaseline({
    title: normalizedTitle,
    noteTypeId: noteTypeId.value,
    headerLayout: headerLayout.value,
    headerPropsJson: normalizedHeaderPropsJson,
  });
  savedDocumentRevision = documentRevision;
  savedMetadataRevision = metadataRevision;
  perfTracker.recordMetric("saveDuration", performance.now() - saveStartedAt);
  console.log(
    "[eden] Editor.save: persisted id=",
    props.entry.id,
    "title=",
    normalizedTitle,
    "content length=",
    content_json.length,
  );
}

function handleNoteTypeChange(nextTypeId: string) {
  const normalizedTypeId = nextTypeId || SYSTEM_TYPE_NOTE_ID;
  const nextNoteType = props.noteTypes.find((noteType) => noteType.id === normalizedTypeId) ?? null;
  noteTypeId.value = normalizedTypeId;
  headerLayout.value = nextNoteType ? resolveNoteTypeHeaderLayout(nextNoteType) : "inline";
  headerProps.value = createDefaultHeaderProps(nextNoteType);
  headerValidationError.value = null;
  isNoteTypeMenuOpen.value = false;
  isTypePickerOpen.value = false;
}

function toggleNoteTypeMenu() {
  isNoteTypeMenuOpen.value = !isNoteTypeMenuOpen.value;
  isTypePickerOpen.value = isNoteTypeMenuOpen.value;
}

function toggleTypePicker() {
  isTypePickerOpen.value = !isTypePickerOpen.value;
}

function openActiveTypeSettings() {
  props.onOpenTypeSettings(noteTypeId.value || SYSTEM_TYPE_NOTE_ID);
  isNoteTypeMenuOpen.value = false;
  isTypePickerOpen.value = false;
}

function handleHeaderPropChange(fieldId: string, value: unknown) {
  headerProps.value = { ...headerProps.value, [fieldId]: value };
}

function focusPrimarySurface() {
  if (!title.value.trim()) {
    titleInputRef.value?.focus();
    titleInputRef.value?.setSelectionRange(0, 0);
    return;
  }

  editor.value?.commands.focus("end");
}

function shouldTrackTypingEvent(event: KeyboardEvent) {
  if (event.ctrlKey || event.metaKey || event.altKey) {
    return false;
  }

  return event.key.length === 1 || TRACKED_EDIT_KEYS.has(event.key);
}

watch(
  [title, noteTypeId, headerLayout, headerProps],
  () => {
    markMetadataDirty();
  },
  { deep: true, flush: "post" },
);

watch(
  () => props.zenMode,
  (isZenMode) => {
    if (!isZenMode) return;

    isNoteTypeMenuOpen.value = false;
    isTypePickerOpen.value = false;
  },
);

watch(
  () => props.entry.id,
  () => {
    hydrateFromEntry(props.entry);
    nextTick(() => {
      window.requestAnimationFrame(() => {
        focusPrimarySurface();
      });
    });
  },
  { immediate: true },
);

watch(
  () => props.entry.content_json,
  (contentJson) => {
    const normalizedIncoming = normalizeContentJson(contentJson);
    if (!editor.value || normalizedIncoming === lastPersistedContentJson || isDirty()) {
      return;
    }

    lastPersistedContentJson = normalizedIncoming;
    isHydrating = true;
    editor.value.commands.setContent(parseContentJson(normalizedIncoming), false);
    resetRevisionBaseline();
    queueMicrotask(() => {
      lastPersistedContentJson = getSerializedEditorContent();
      lastPersistedMarkdown = getSerializedEditorMarkdown();
      isHydrating = false;
      emitLiveCharCount();
    });
  },
);

watch(
  () => props.entry.updated_at,
  () => {
    if (!isDirty()) {
      savedDocumentRevision = documentRevision;
      savedMetadataRevision = metadataRevision;
    }
  },
);

watch(
  [
    () => props.entry.title,
    () => props.entry.type_id,
    () => props.entry.header_layout,
    () => props.entry.header_props_json,
    () => props.noteTypes,
  ],
  ([nextTitle, nextTypeId, nextHeaderLayout, nextHeaderPropsJson]) => {
    if (isDirty()) return;

    const normalizedTypeId = nextTypeId ?? SYSTEM_TYPE_NOTE_ID;
    const nextNoteType = props.noteTypes.find((noteType) => noteType.id === normalizedTypeId) ?? null;
    const normalizedHeaderPropsJson = normalizeHeaderPropsJson(nextHeaderPropsJson);

    isHydrating = true;
    title.value = getEditableEntryTitle(nextTitle, nextHeaderPropsJson);
    noteTypeId.value = normalizedTypeId;
    headerLayout.value = nextHeaderLayout ?? resolveNoteTypeHeaderLayout(nextNoteType);
    headerProps.value = safeParseHeaderProps(nextNoteType, nextHeaderPropsJson);
    headerValidationError.value = null;
    updatePersistedMetadataBaseline({
      title: nextTitle,
      noteTypeId: normalizedTypeId,
      headerLayout: nextHeaderLayout ?? "default",
      headerPropsJson: normalizedHeaderPropsJson,
    });

    queueMicrotask(() => {
      isHydrating = false;
    });
  },
);

watchEffect((onCleanup) => {
  if (!editor.value) return;

  const dom = editor.value.view.dom;

  const handleKeyDown = (event: KeyboardEvent) => {
    if (!shouldTrackTypingEvent(event)) return;

    const startedAt = performance.now();
    requestAnimationFrame(() => {
      perfTracker.recordMetric("inputToNextPaint", performance.now() - startedAt);
    });
  };

  dom.addEventListener("keydown", handleKeyDown, true);

  let longTaskObserver: PerformanceObserver | null = null;
  if (typeof window.PerformanceObserver === "function") {
    try {
      longTaskObserver = new PerformanceObserver((entryList) => {
        for (const entry of entryList.getEntries()) {
          perfTracker.recordLongTask(entry.duration);
        }
      });
      longTaskObserver.observe({ entryTypes: ["longtask"] });
    } catch {
      longTaskObserver = null;
    }
  }

  window.__edenPerf = perfTracker;

  onCleanup(() => {
    dom.removeEventListener("keydown", handleKeyDown, true);
    longTaskObserver?.disconnect();
    if (window.__edenPerf === perfTracker) {
      delete window.__edenPerf;
    }
  });
});

watchEffect((onCleanup) => {
  if (!editor.value) return;

  const handleKeyDown = (event: KeyboardEvent) => {
    if ((event.metaKey || event.ctrlKey) && event.key === "s") {
      event.preventDefault();
      void save();
    }
  };

  document.addEventListener("keydown", handleKeyDown);
  onCleanup(() => document.removeEventListener("keydown", handleKeyDown));
});

watchEffect((onCleanup) => {
  if (!editor.value) return;

  const handleClick = (event: MouseEvent) => {
    const target = event.target as HTMLElement;
    const wikilink = target.closest("span[data-id]");
    if (wikilink) {
      const entryId = wikilink.getAttribute("data-id");
      if (entryId) props.onNavigate(entryId);
    }
  };

  const dom = editor.value.view.dom;
  dom.addEventListener("click", handleClick);
  onCleanup(() => dom.removeEventListener("click", handleClick));
});

watchEffect((onCleanup) => {
  if (!isNoteTypeMenuOpen.value) return;

  const handlePointerDown = (event: MouseEvent) => {
    if (noteTypeMenuRef.value?.contains(event.target as Node)) return;
    isNoteTypeMenuOpen.value = false;
    isTypePickerOpen.value = false;
  };

  const handleEscape = (event: KeyboardEvent) => {
    if (event.key === "Escape") {
      isNoteTypeMenuOpen.value = false;
      isTypePickerOpen.value = false;
    }
  };

  window.addEventListener("mousedown", handlePointerDown);
  window.addEventListener("keydown", handleEscape);
  onCleanup(() => {
    window.removeEventListener("mousedown", handlePointerDown);
    window.removeEventListener("keydown", handleEscape);
  });
});

onBeforeUnmount(() => {
  if (isDirty()) {
    // Async save fire-and-forget — Vue не поддерживает await в unmount хуке,
    // мы не можем заблокировать destroy. Catch на rejection чтобы не было
    // unhandled rejection (save() сам логирует через saveConflict, но к
    // моменту его resolve компонент уже unmounted — никто не увидит).
    void save().catch((e) => {
      console.error("[eden] save on unmount failed:", e);
    });
  }

  clearScheduledWork();
  if (window.__edenPerf === perfTracker) {
    delete window.__edenPerf;
  }
});
</script>
