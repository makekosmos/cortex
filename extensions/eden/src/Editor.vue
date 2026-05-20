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
import TaskList from "@tiptap/extension-task-list";
import { all, createLowlight } from "lowlight";
import Typography from "@tiptap/extension-typography";
import type { Editor as TiptapEditor, Range } from "@tiptap/vue-3";
import { Wikilink } from "./Wikilink";
import { TaskItemWithId } from "./TaskItemWithId";
import { edenApi } from "@/lib/edenApi";
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

// 300ms — баланс между «не пишем на каждую клавишу» и «минимизируем окно
// потери при жёстком kill процесса». ARK — локальный SQLite, write дешёвый
// (~1ms), запас 800ms был unnecessary. Плюс есть flush на blur /
// visibilitychange / beforeunload — обычные сценарии (закрытие окна,
// переключение на другую заметку, alt-tab) сохраняют немедленно.
const AUTOSAVE_DEBOUNCE_MS = 300;
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
// lastPersistedMarkdown был удалён 2026-05-19 вместе с OR в matchesPersistedState.
// `@tiptap/markdown` v3 сменил API, поле всегда оставалось "" → false-positive
// matches → save'ы пропускались. ContentJson — единственная истина для diff'а.
let lastPersistedTitle = props.entry.title;
let lastPersistedNoteTypeId = props.entry.type_id ?? SYSTEM_TYPE_NOTE_ID;
let lastPersistedHeaderLayout = props.entry.header_layout ?? "default";
let lastPersistedHeaderPropsJson = normalizeHeaderPropsJson(props.entry.header_props_json);

// Snapshot taskItem-ов на последний успешно засинканный в ARK момент.
// Key — taskId (UUID из TipTap attribute), value — текст и checked-флаг.
// Diff в `syncTaskItemsToArk()` против текущего состояния документа решает
// создать/обновить/soft-delete task_obj. Map хешируется по строке для O(1)
// lookup, snapshot фиксируется только после успешного sync (см. ниже).
type TaskSnapshotEntry = { title: string; checked: boolean };
let lastTaskSnapshot: Map<string, TaskSnapshotEntry> = walkTaskItemsInJson(lastPersistedContentJson);

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
  // TaskList + TaskItemWithId: чекбокс-задачи внутри заметки. taskId UUID
  // привязывает каждую ноду к task_obj в ARK — после save заметки diff'ается
  // в Editor.vue::syncTaskItems → upsert/soft-delete в Delphi.
  TaskList,
  TaskItemWithId.configure({ nested: true }),
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
            title: "Задача",
            icon: "☐",
            command: ({ editor, range }: { editor: TiptapEditor; range: Range }) =>
              editor.chain().focus().deleteRange(range).toggleTaskList().run(),
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
  // Flush save при потере фокуса редактора. Юзер кликнул в title, нажал
  // ESC чтобы выйти из zen, или переключился в sidebar — гарантируем что
  // его последние правки уже в ARK, не ждём debounce 300ms.
  onBlur: () => {
    void flushAutoSave();
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
  // @tiptap/markdown v3 API. Раньше было `storage.markdown.getMarkdown()`,
  // в v3 manager переехал в `editor.markdown` (или `editor.getMarkdown()`).
  // Не критично для сохранения (contentJson авторитетен), используется только
  // для экспорта/копирования если когда-нибудь понадобится. Defensive `?.`.
  const ed = editor.value as unknown as { getMarkdown?: () => string } | null;
  return ed?.getMarkdown?.() ?? "";
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

    if (
      matchesPersistedState({
        title: normalizedTitle,
        noteTypeId: noteTypeId.value,
        headerLayout: headerLayout.value,
        headerPropsJson: normalizedHeaderPropsJson,
        contentJson,
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

/**
 * Принудительно сохранить сейчас, не дожидаясь debounce таймера. Вешается
 * на события «вот-вот может случиться потеря контекста»:
 *
 *   - blur редактора (фокус ушёл; юзер может закрыть окно следом)
 *   - visibilitychange → hidden (свернул окно / переключился в другое app)
 *   - beforeunload (Electron начал закрывать окно)
 *
 * Не fire-and-forget — возвращает Promise<void>, чтобы caller'ы могли
 * await'ить в критичных местах (например, `onBeforeUnmount` в Vue не
 * поддерживает await, но мы хотя бы запустим save до того как Vue начнёт
 * teardown).
 */
async function flushAutoSave(): Promise<void> {
  if (autosaveTimer !== null) {
    window.clearTimeout(autosaveTimer);
    autosaveTimer = null;
  }
  if (!isDirty()) return;
  await save();
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
}) {
  const metadataMatches =
    options.title === lastPersistedTitle &&
    options.noteTypeId === lastPersistedNoteTypeId &&
    options.headerLayout === lastPersistedHeaderLayout &&
    options.headerPropsJson === lastPersistedHeaderPropsJson;

  // ContentJson — единственная авторитетная истина для «изменился ли doc».
  // Раньше тут был OR с markdown (`options.markdown === lastPersistedMarkdown`),
  // но `@tiptap/markdown` v3 изменил API: было `storage.markdown.getMarkdown()`,
  // стало `editor.getMarkdown()`. `getSerializedEditorMarkdown()` поэтому
  // всегда возвращал "", lastPersistedMarkdown тоже "" → OR всегда true →
  // `schedulePersistedStateReconciliation` помечал state как persisted без
  // реальной записи в ARK → save() при autosave timer'е возвращался рано
  // (isDirty=false). Все правки пользователя терялись. См. 2026-05-19
  // user report «не сохраняется ни одна заметка в деве».
  return metadataMatches && options.contentJson === lastPersistedContentJson;
}

function hydrateFromEntry(entry: Entry) {
  const nextTypeId = entry.type_id ?? SYSTEM_TYPE_NOTE_ID;
  const nextNoteType = props.noteTypes.find((noteType) => noteType.id === nextTypeId) ?? null;
  const nextContentJson = normalizeContentJson(entry.content_json);

  console.log(
    "[eden] Editor.hydrateFromEntry id=",
    entry.id,
    "type=",
    nextTypeId,
    "content[:200]=",
    nextContentJson.slice(0, 200),
  );

  isHydrating = true;
  title.value = getEditableEntryTitle(entry.title, entry.header_props_json);
  noteTypeId.value = nextTypeId;
  headerLayout.value = entry.header_layout ?? "default";
  headerProps.value = safeParseHeaderProps(nextNoteType, entry.header_props_json);
  headerValidationError.value = null;
  saveConflict.value = null;
  lastPersistedContentJson = nextContentJson;
  // Task snapshot — derive из загружаемого contentJson. Это baseline для
  // diff'а: ничего не должно syncn'уться на первом save после открытия
  // заметки, кроме реальных изменений сделанных юзером.
  lastTaskSnapshot = walkTaskItemsInJson(nextContentJson);
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
    isHydrating = false;
    emitLiveCharCount();
  });
}

/**
 * Обходит ProseMirror-doc (в виде JSON) и собирает все taskItem ноды в
 * Map<taskId, {title, checked}>. Title — конкатенация текста внутри ноды
 * (paragraph children). Если taskId отсутствует — нода пропускается (UUID
 * проставит appendTransaction-плагин на следующей правке). Если nested
 * taskList — рекурсивно зайдёт через children.
 *
 * O(N) по числу нод doc'а, типичная заметка <1000 нод → ≪1ms.
 */
function walkTaskItemsInJson(contentJson: string): Map<string, TaskSnapshotEntry> {
  const out = new Map<string, TaskSnapshotEntry>();
  let doc: unknown;
  try {
    doc = JSON.parse(contentJson);
  } catch {
    return out;
  }
  walkNode(doc, out);
  return out;
}

function walkNode(node: unknown, out: Map<string, TaskSnapshotEntry>): void {
  if (!node || typeof node !== "object") return;
  const n = node as { type?: string; attrs?: { taskId?: string; checked?: boolean }; content?: unknown[] };
  if (n.type === "taskItem" && n.attrs?.taskId) {
    out.set(n.attrs.taskId, {
      title: extractTextContent(n.content ?? []).trim(),
      checked: Boolean(n.attrs.checked),
    });
  }
  if (Array.isArray(n.content)) {
    for (const child of n.content) walkNode(child, out);
  }
}

function extractTextContent(content: unknown[]): string {
  let out = "";
  for (const child of content) {
    if (!child || typeof child !== "object") continue;
    const c = child as { type?: string; text?: string; content?: unknown[] };
    if (c.type === "text" && typeof c.text === "string") {
      out += c.text;
    } else if (Array.isArray(c.content)) {
      out += extractTextContent(c.content);
    }
  }
  return out;
}

/**
 * Diff'ает текущее состояние taskItem'ов против `lastTaskSnapshot` и шлёт
 * upsert/soft-delete в ARK. Вызывается ПОСЛЕ успешного `props.onSave(...)`,
 * чтобы task_obj не создавались для не-сохранённой заметки.
 *
 * Sync failure НЕ пробрасывается — мы логируем warn и НЕ обновляем snapshot
 * для упавших операций (упавшие задачи ретраются на следующем save'е).
 * Это гарантирует что временная недоступность ARK не блокирует save заметки
 * и не теряет sync state.
 */
async function syncTaskItemsToArk(): Promise<void> {
  const currentSnapshot = walkTaskItemsInJson(lastPersistedContentJson);
  const previous = lastTaskSnapshot;
  const noteId = props.entry.id;

  const upsertSuccesses: Array<[string, TaskSnapshotEntry]> = [];
  const deleteSuccesses: string[] = [];

  for (const [taskId, current] of currentSnapshot) {
    const prev = previous.get(taskId);
    if (prev && prev.title === current.title && prev.checked === current.checked) {
      // Без изменений — ничего не шлём.
      upsertSuccesses.push([taskId, current]);
      continue;
    }
    try {
      await edenApi.upsertTaskFromNote({
        taskId,
        title: current.title || "Без названия",
        isCompleted: current.checked,
        sourceNoteId: noteId,
      });
      upsertSuccesses.push([taskId, current]);
    } catch (err) {
      console.warn("[eden] task sync upsert failed", taskId, err);
      // Не обновляем snapshot для taskId → следующий save повторит попытку.
    }
  }

  for (const taskId of previous.keys()) {
    if (currentSnapshot.has(taskId)) continue;
    try {
      await edenApi.softDeleteTaskFromNote(taskId);
      deleteSuccesses.push(taskId);
    } catch (err) {
      console.warn("[eden] task sync delete failed", taskId, err);
    }
  }

  // Реконструируем snapshot из successful operations:
  // - все upsert'ы (включая без изменений) → попадают
  // - все delete'ы → исключаются
  // - upserts с failure → остаются с previous значением (ретрай на следующий save)
  const nextSnapshot = new Map<string, TaskSnapshotEntry>();
  for (const [taskId, entry] of upsertSuccesses) {
    nextSnapshot.set(taskId, entry);
  }
  // Failed upserts должны сохранить previous значение, чтобы diff на следующем
  // save'е увидел разницу с current и повторил upsert.
  for (const [taskId, prevEntry] of previous) {
    if (nextSnapshot.has(taskId)) continue;
    if (deleteSuccesses.includes(taskId)) continue;
    if (!currentSnapshot.has(taskId)) continue; // объект уже отсутствует и удалён успешно
    nextSnapshot.set(taskId, prevEntry);
  }
  lastTaskSnapshot = nextSnapshot;
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

  if (
    matchesPersistedState({
      title: normalizedTitle,
      noteTypeId: noteTypeId.value,
      headerLayout: headerLayout.value,
      headerPropsJson: normalizedHeaderPropsJson,
      contentJson: content_json,
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
  // Task sync — после того как заметка успешно сохранена в ARK.
  // Если упадёт — лог в warn, но save заметки уже committed. Snapshot
  // обновляется внутри syncTaskItemsToArk только для successful operations
  // (failed upsert'ы ретраятся на следующем save'е).
  await syncTaskItemsToArk();
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

// Save flush listeners — отдельный watchEffect, без conditional return.
// Раньше эти listener'ы лежали в noteTypeMenu watchEffect выше, который
// guard'ит `if (!isNoteTypeMenuOpen.value) return;` — flush'и не работали
// пока меню типов было закрыто (т.е. почти всегда). UI test 2026-05-19
// поймал это: visibilitychange dispatch → flush не выполнялся → save
// не происходил.
watchEffect((onCleanup) => {
  // Flush save при ЛЮБОМ visibilitychange (→hidden / →visible). Семантически
  // важен только переход в hidden (alt-tab, minimize), но flush на → visible
  // безопасен: если isDirty=false (типичный случай при возврате фокуса),
  // flushAutoSave — no-op. Не проверяем `document.visibilityState` чтобы
  // тесты не зависели от prototype override этого read-only accessor'а.
  const handleVisibilityChange = () => {
    void flushAutoSave();
  };
  document.addEventListener("visibilitychange", handleVisibilityChange);

  // Flush save при beforeunload (Electron начал закрывать окно). У нас есть
  // ещё onBeforeUnmount fire-and-forget save, но beforeunload срабатывает
  // РАНЬШЕ (до того как Vue начинает teardown компонента), даёт чуть больше
  // времени async-операции долететь до ARK через WS.
  const handleBeforeUnload = () => {
    void flushAutoSave();
  };
  window.addEventListener("beforeunload", handleBeforeUnload);

  onCleanup(() => {
    document.removeEventListener("visibilitychange", handleVisibilityChange);
    window.removeEventListener("beforeunload", handleBeforeUnload);
  });
});

onBeforeUnmount(() => {
  // Fire-and-forget flush — Vue не поддерживает await в unmount хуке.
  // `flushAutoSave` cancels pending timer + calls save() inline; в большинстве
  // случаев blur/visibilitychange/beforeunload уже отработали раньше, и
  // isDirty() здесь false → no-op. Catch на rejection — нет unhandled.
  void flushAutoSave().catch((e) => {
    console.error("[eden] save on unmount failed:", e);
  });

  clearScheduledWork();
  if (window.__edenPerf === perfTracker) {
    delete window.__edenPerf;
  }
});
</script>
