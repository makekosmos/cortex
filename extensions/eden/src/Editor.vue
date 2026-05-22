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
              <div class="note-type-menu-submenu" @mouseenter="isTypePickerOpen = true">
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
                    :class="[
                      'note-type-menu-item',
                      'note-type-submenu-item',
                      noteType.id === noteTypeId && 'is-active',
                    ]"
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
    <div ref="contentAreaRef" class="editor-content-area" @mousedown="onContentMouseDown">
      <div class="editor-rail editor-content-rail">
        <EditorContent :editor="editor ?? null" />
      </div>
      <BlockSelectionOverlay :rect="blockSelection.dragRect.value" />
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
import { TaskRef } from "./TaskRef";
import { TrailingParagraph } from "./TrailingParagraph";
import { edenApi } from "@/lib/edenApi";
import { useBlockSelection } from "@/composables/useBlockSelection";
import BlockSelectionOverlay from "@/components/BlockSelectionOverlay.vue";
import { TextSelection } from "@tiptap/pm/state";
import { useToast } from "@kosmos/visuals";
import {
  BlockSelectionDecoration,
  blockSelectionPluginKey,
  buildBlockSelectionDecorations,
} from "./BlockSelectionDecoration";
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
    props.noteTypes.find(
      (noteType) => noteType.id === (props.entry.type_id ?? SYSTEM_TYPE_NOTE_ID),
    ) ?? null,
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

const typePickerOptions = computed(() =>
  props.noteTypes.filter((noteType) => Boolean(noteType.id)),
);

function getNoteTypeIconSrc(noteType: NoteType | null) {
  return noteType?.icon ? objectIconUri(noteType.icon) : "";
}

const contentAreaRef = useTemplateRef<HTMLDivElement>("contentAreaRef");
const blockSelection = useBlockSelection();
const toast = useToast();

// Флаг ставится window-level mousedown handler'ом (capture-фаза) когда
// он очистил block selection. onContentMouseDown в bubble-фазе видит флаг
// и отказывается стартовать новый drag-tracking — это была «clearing
// click», без него jitter мыши + CROSSING_MIN_PX=8 запускает drag через
// границу блока и снова выделяет соседние задачи.
let justClearedSelection = false;

// Block-selection mouse flow (Anytype canonical pattern):
// 1. mousedown: НЕ preventDefault'им — PM нормально обрабатывает (focus
//    + caret) если клик на текст. Запускаем tracking — никаких visual
//    изменений пока mouse не сдвинется на 20px.
// 2. mousemove: до threshold — пропускаем (юзер может просто кликать).
//    После threshold ("activated") — блюрим PM + collapse selection,
//    block-selection layer берёт верх. Дальше каждый mousemove обновляет
//    drag rect + selected blocks.
// 3. mouseup: finishDrag. Если drag не активировался — selection не
//    тронута, PM caret где поставил браузер.
//
// Это даёт: обычный click → PM как раньше, drag → block selection.
// Никакого preventDefault на mousedown → нет broken intermediate state.
function onContentMouseDown(e: MouseEvent): void {
  if (!editor.value || !contentAreaRef.value) return;
  const target = e.target as HTMLElement | null;
  if (!target) return;
  // Global mousedown handler в capture-фазе уже мог очистить block
  // selection до того как событие дошло до bubble-фазы — тогда hasSelection
  // здесь false, но юзер сделал «clearing click»: новый drag запускать
  // нельзя (минимальный jitter мыши + CROSSING_MIN_PX=8 → активация
  // через границу блока → задача под курсором + соседняя оказались бы
  // выделены).
  if (blockSelection.hasSelection.value || justClearedSelection) {
    blockSelection.clearSelection();
    justClearedSelection = false;
    return;
  }
  // Drag tracking — только LMB и только на действительно interactive
  // controls (buttons / checkboxes). НЕ фильтруем input/textarea — там
  // браузер сам обработает focus + caret по click default'у, а drag
  // через границу блока должен запуститься (юзер тянет от текста
  // задачи в следующий блок).
  if (e.button !== 0) return;
  if (target.closest("button, [role=button], [role=checkbox]")) return;
  // Клик в пустое пространство ниже последнего блока (на сам
  // .editor-content-area или .editor-rail, не на .ProseMirror и его
  // потомков) → фокус в конец документа. Без этого пользователь не может
  // «продолжить писать», если последний блок — task с inline input'ом.
  const clickedEmptySpace =
    target === contentAreaRef.value ||
    target.classList.contains("editor-rail") ||
    (target.classList.contains("ProseMirror") && e.target === target);
  if (clickedEmptySpace) {
    const pmEl = contentAreaRef.value.querySelector(".ProseMirror") as HTMLElement | null;
    if (pmEl) {
      const rect = pmEl.getBoundingClientRect();
      // Клик ниже последнего блока — focus в самый конец.
      if (e.clientY > rect.bottom - 4) {
        e.preventDefault();
        editor.value.chain().focus("end").run();
        return;
      }
    }
  }
  blockSelection.startTracking(editor.value, e.clientX, e.clientY, contentAreaRef.value);
  window.addEventListener("mousemove", onWindowMouseMove);
  window.addEventListener("mouseup", onWindowMouseUp, { once: true });
}

function onWindowMouseMove(e: MouseEvent): void {
  const result = blockSelection.updateDrag(e.clientX, e.clientY);
  if (result === "activated" && editor.value) {
    blockSelection.collapseEditorSelection(editor.value);
  }
  if (result !== null) {
    e.preventDefault();
    // Continuous removeAllRanges пока drag активен. Browser/PM пытаются
    // переустановить native Range на каждый mousemove (mouse button
    // held = drag mode), removeAllRanges на activation один раз
    // недостаточно — текстовое выделение возвращается. Чистим каждый
    // фрейм пока drag не отпустят.
    const sel = window.getSelection();
    if (sel && sel.rangeCount > 0) sel.removeAllRanges();
  }
}

function onWindowMouseUp(): void {
  blockSelection.finishDrag();
  window.removeEventListener("mousemove", onWindowMouseMove);
  // Note: НЕ форсим focus здесь. Если был click без drag — PM сам уже
  // обработал mousedown и поставил caret. Если был drag — юзер сам
  // решит куда кликнуть дальше.
}

// Keyboard: Esc → clear selection; Delete/Backspace → delete selected blocks;
// Ctrl+C / Ctrl+X → copy/cut выделенного как markdown; Ctrl+C при обычной PM
// selection (включая Ctrl+A) — копирует выделение целиком как markdown.
function onWindowKeyDown(e: KeyboardEvent): void {
  if (!editor.value) return;

  if (blockSelection.hasSelection.value) {
    if (e.key === "Escape") {
      e.preventDefault();
      blockSelection.clearSelection();
      return;
    }
    if (e.key === "Delete" || e.key === "Backspace") {
      e.preventDefault();
      void blockSelection.deleteSelected(editor.value, edenApi.softDeleteTask);
      return;
    }
    if ((e.ctrlKey || e.metaKey) && e.code === "KeyC") {
      e.preventDefault();
      const md = serializeSelectedBlocksAsMarkdown();
      if (md) {
        void navigator.clipboard.writeText(md);
        toast.show({
          message: `Скопировано ${blockSelection.selectedPositions.value.size} ${pluralizeBlocks(blockSelection.selectedPositions.value.size)}`,
          tone: "success",
        });
      }
      return;
    }
    if ((e.ctrlKey || e.metaKey) && e.code === "KeyX") {
      e.preventDefault();
      const count = blockSelection.selectedPositions.value.size;
      const md = serializeSelectedBlocksAsMarkdown();
      if (md) {
        void navigator.clipboard.writeText(md);
        void blockSelection.deleteSelected(editor.value, edenApi.softDeleteTask);
        toast.show({
          message: `Вырезано ${count} ${pluralizeBlocks(count)}`,
          tone: "success",
        });
      }
      return;
    }
    return;
  }

  // Ctrl+A → активируем block selection (как drag-select), не PM-native
  // selectAll. Это даёт визуальный overlay по всем блокам + унифицирует
  // copy/cut путь через serializeSelectedBlocksAsMarkdown.
  //
  // Срабатывает если target внутри editor-wrapper (включая title input,
  // task-ref inputs, итд) — НЕ только если editor body имеет focus.
  // Иначе из title input native Ctrl+A select'ил бы текст в input'е, а
  // Ctrl+C дал бы plain text title, не markdown.
  //
  // NB: используем `e.code === "KeyA"` (физическая клавиша) — `e.key === "a"`
  // ломается на русской раскладке (там та же клавиша возвращает "ф").
  // Это инвариант для ВСЕХ ctrl-shortcut'ов с латинской буквой,
  // см. docs-site/agents/forbidden.md → UI / раскладка.
  if ((e.ctrlKey || e.metaKey) && e.code === "KeyA") {
    const target = e.target as HTMLElement | null;
    const inEditorScope = !!target?.closest(".editor-wrapper");
    if (!inEditorScope) return;
    e.preventDefault();
    // Перенесём focus в editor body (если был в input) — иначе следующий
    // Ctrl+C native'ом сработает по input'у, не по нашему handler'у.
    editor.value.commands.focus();
    blockSelection.selectAll(editor.value);
    return;
  }

  // PM range selection branch — range-select→Ctrl+C / Ctrl+X.
  // Срабатывает только если editor фокусирован (иначе юзер в title input
  // или другом UI — пускаем native handle).
  if ((e.ctrlKey || e.metaKey) && (e.code === "KeyC" || e.code === "KeyX")) {
    const sel = editor.value.state.selection;
    if (sel.empty) return;
    if (!editor.value.view.hasFocus()) return;
    e.preventDefault();
    const isCut = e.code === "KeyX";
    const md = serializePMSelectionAsMarkdown();
    if (md) {
      void navigator.clipboard.writeText(md);
      if (isCut) editor.value.commands.deleteSelection();
      toast.show({
        message: isCut ? "Вырезано в Markdown" : "Скопировано в Markdown",
        tone: "success",
      });
    }
  }
}

function pluralizeBlocks(n: number): string {
  const mod10 = n % 10;
  const mod100 = n % 100;
  if (mod10 === 1 && mod100 !== 11) return "блок";
  if (mod10 >= 2 && mod10 <= 4 && (mod100 < 12 || mod100 > 14)) return "блока";
  return "блоков";
}

/**
 * Walk JSON doc, для каждого taskRef патчит attrs.titleSnapshot значением
 * из DOM input'а — markdown renderer (см. TaskRef.ts) рендерит
 * `- [ ] ${titleSnapshot}`. Без патча attr пустой и markdown получает
 * `- [ ] ` (источник истины title живёт в ARK, не в attrs).
 */
function patchTaskRefSnapshots(
  json: { type?: string; content?: unknown[]; attrs?: Record<string, unknown> },
  viewDom: HTMLElement,
): void {
  const walk = (node: {
    type?: string;
    content?: unknown[];
    attrs?: Record<string, unknown>;
  }): void => {
    if (!node) return;
    if (node.type === "taskRef" && node.attrs?.taskId) {
      const input = viewDom.querySelector(
        `[data-task-id="${node.attrs.taskId as string}"] .task-ref-title-input`,
      ) as HTMLInputElement | null;
      if (input) node.attrs = { ...node.attrs, titleSnapshot: input.value };
    }
    if (Array.isArray(node.content)) {
      for (const child of node.content) walk(child as never);
    }
  };
  walk(json);
}

function serializeSelectedBlocksAsMarkdown(): string {
  if (!editor.value) return "";
  const view = editor.value.view;
  const doc = view.state.doc;
  const docJson = doc.toJSON() as { type: string; content?: unknown[] };
  patchTaskRefSnapshots(docJson, view.dom as HTMLElement);
  const positionsSet = blockSelection.selectedPositions.value;
  if (positionsSet.size === 0) return "";
  const filtered: unknown[] = [];
  let runningPos = 0;
  for (let i = 0; i < doc.childCount; i++) {
    const child = doc.child(i);
    if (positionsSet.has(runningPos) && docJson.content?.[i]) {
      filtered.push(docJson.content[i]);
    }
    runningPos += child.nodeSize;
  }
  if (filtered.length === 0) return "";
  const sliceJson = { type: "doc", content: filtered };
  return (
    editor.value as unknown as { markdown: { serialize: (j: unknown) => string } }
  ).markdown.serialize(sliceJson);
}

function serializePMSelectionAsMarkdown(): string {
  if (!editor.value) return "";
  const view = editor.value.view;
  const sel = view.state.selection;
  if (sel.empty) return "";
  const doc = view.state.doc;
  let sliceJson: { type: string; content?: unknown[] };
  if (sel.from <= 0 && sel.to >= doc.content.size) {
    sliceJson = doc.toJSON() as { type: string; content?: unknown[] };
  } else {
    const sliceContent = doc.slice(sel.from, sel.to).content.toJSON() as unknown[] | null;
    sliceJson = { type: "doc", content: sliceContent ?? [] };
  }
  patchTaskRefSnapshots(sliceJson, view.dom as HTMLElement);
  return (
    editor.value as unknown as { markdown: { serialize: (j: unknown) => string } }
  ).markdown.serialize(sliceJson);
}

/**
 * @deprecated Legacy plain-text сериализатор — оставлен для обратной совместимости
 * если кто-то ещё импортирует. Новый путь — markdown через render handlers.
 */
function serializeSelectedBlocks(): string {
  if (!editor.value) return "";
  const view = editor.value.view;
  const positions = Array.from(blockSelection.selectedPositions.value).sort((a, b) => a - b);
  const parts: string[] = [];
  for (const pos of positions) {
    const node = view.state.doc.nodeAt(pos);
    if (!node) continue;
    if (node.type.name === "taskRef") {
      const dom = view.nodeDOM(pos) as HTMLElement | null;
      const input = dom?.querySelector(".task-ref-title-input") as HTMLInputElement | null;
      parts.push(`- [ ] ${input?.value ?? ""}`);
    } else {
      parts.push(node.textContent || "");
    }
  }
  return parts.join("\n");
}

// Container class toggle — `kepler-block-select-active` ставится когда
// активен drag ИЛИ есть persisted block selection. CSS этого класса
// отключает `::selection` и `user-select` в .ProseMirror, чтобы native
// text selection не рисовался параллельно с нашим block-overlay
// (это была главная косметическая проблема первой итерации).
watch(
  () => blockSelection.dragRect.value !== null || blockSelection.hasSelection.value,
  (active) => {
    if (!contentAreaRef.value) return;
    contentAreaRef.value.classList.toggle("kepler-block-select-active", active);
  },
  { flush: "post" },
);

// Reactive applier: следим за selectedPositions, диспатчим tr с новым
// DecorationSet'ом в BlockSelectionDecoration plugin. PM сам управляет
// классами через decorations API — не стирает их на re-render (vs direct
// DOM mutation, которая ломалась после blur/любого PM update).
watch(
  () => blockSelection.selectedPositions.value,
  (selected) => {
    if (!editor.value) return;
    const view = editor.value.view;
    const newDecos = buildBlockSelectionDecorations(view.state.doc, selected);
    const tr = view.state.tr.setMeta(blockSelectionPluginKey, newDecos);
    view.dispatch(tr);
  },
  { deep: false, flush: "post" },
);

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
  // trailingNode: false — StarterKit'овский TrailingNode добавлял
  // paragraph после ЛЮБОГО atom-блока (включая taskRef), конфликтуя с
  // TaskRef.commitAndCreateNew (см. eden-taskref-pm-traps.md ловушка #2).
  // Вместо него используем кастомный TrailingParagraph ниже — он
  // гарантирует empty paragraph именно в самом конце документа, что
  // нужно для UX «всегда есть куда поставить курсор после последнего
  // блока», но без поломки TaskRef-flow.
  StarterKit.configure({ codeBlock: false, trailingNode: false }),
  TrailingParagraph,
  Markdown,
  Placeholder.configure({ placeholder: "Начни писать что-нибудь интересное..." }),
  EdenCodeBlock.configure({
    lowlight,
    enableTabIndentation: true,
    tabSize: 4,
    defaultLanguage: null,
  }),
  // TaskRef: atomic block node, рендерит task_obj live из ARK через NodeView.
  // Pattern B: source of truth — task_obj, content_json хранит только taskId.
  // Bidir sync с Delphi работает через ARK object_upserted events.
  TaskRef.configure({
    // Closure читает текущий entry.id — если юзер откроет другую заметку,
    // input rule подхватит новый id без re-init Editor.
    getSourceNoteId: () => props.entry.id,
  }),
  // PM plugin для block-selection декораций. Заменяет direct DOM mutation
  // (которое стиралось PM на re-render'е, особенно после blur). PM сам
  // переапплицирует декорации после каждой transaction'и.
  BlockSelectionDecoration,
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
            command: async ({ editor, range }: { editor: TiptapEditor; range: Range }) => {
              // 1) deleteRange сразу (синхронно), чтобы /задача-текст исчез
              //    пока создаём task_obj. UX: пользователь не видит «зависший /».
              editor.chain().focus().deleteRange(range).run();
              try {
                const taskId = await edenApi.createTask(props.entry.id, "");
                editor.commands.insertTaskRef(taskId);
              } catch (err) {
                console.warn("[eden] /задача создание task_obj упало:", err);
              }
            },
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

// Test-only: expose editor instance для PM introspection в e2e specs.
// Не несёт runtime overhead'а, но позволяет тестам читать selection state.
watch(
  editor,
  (instance) => {
    if (typeof window !== "undefined") {
      (window as unknown as { __edenEditor?: unknown }).__edenEditor = instance ?? undefined;
    }
  },
  { immediate: true },
);

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

function getSuggestionClientRect(renderProps: {
  clientRect?: (() => DOMRect | null) | null;
  editor: TiptapEditor;
  range: Range;
}) {
  if (renderProps.clientRect) {
    return renderProps.clientRect;
  }

  return () => {
    const { left, right, top, bottom } = renderProps.editor.view.coordsAtPos(
      renderProps.range.from,
    );
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
  const rawContentJson = normalizeContentJson(entry.content_json);
  // Migration Pattern C → Pattern B: если в content_json есть legacy taskItem
  // ноды — перепишем их в taskRef. Lazy on-open, idempotent. Save через
  // обычный autosave flow на ближайшей правке (markDocumentDirty флагнет
  // ниже). Если миграция произошла — обновим persisted baseline тоже, иначе
  // diff не увидит изменения и save не побежит.
  const migratedContentJson = migrateTaskItemsToTaskRefs(rawContentJson);
  const nextContentJson = migratedContentJson ?? rawContentJson;

  console.log(
    "[eden] Editor.hydrateFromEntry id=",
    entry.id,
    "type=",
    nextTypeId,
    "migrated=",
    migratedContentJson !== null,
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
  // Если миграция изменила JSON — фиксируем как baseline СТАРЫЙ raw, чтобы
  // diff в save() увидел разницу и записал migrated в ARK на ближайшем
  // flushAutoSave. Иначе baseline = migrated и save'а не будет.
  lastPersistedContentJson = migratedContentJson !== null ? rawContentJson : nextContentJson;
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
    // NB: при миграции baseline остаётся СТАРЫМ rawContentJson — не reset'им
    // его из текущего editor content, иначе diff не сработает. Без миграции
    // baseline = current editor content (как было до Pattern B).
    if (migratedContentJson === null) {
      lastPersistedContentJson = getSerializedEditorContent();
    }
    isHydrating = false;
    emitLiveCharCount();
    if (migratedContentJson !== null) {
      // Migration → save сразу через autosave путь. markDocumentDirty
      // флагнет flushAutoSave чтобы isDirty() вернул true и save() побежал.
      markDocumentDirty();
      void flushAutoSave();
    }
  });
}

/**
 * Migration legacy taskItem → taskRef. До 2026-05-20 Eden хранил задачи как
 * TipTap taskItem ноды с UUID `taskId` атрибутом (Pattern C). Pattern B
 * (taskRef) заменил это: source of truth — task_obj в ARK, content_json
 * хранит только id. Эта функция вызывается в `hydrateFromEntry`: сканирует
 * JSON, заменяет каждый taskItem на taskRef, возвращает новый JSON или null
 * если миграции не требуется (idempotent).
 *
 * Task_obj для каждого taskItem уже создан Pattern C кодом на save —
 * ничего ARK-side не делаем, просто rewriting content_json.
 */
function migrateTaskItemsToTaskRefs(contentJson: string): string | null {
  let doc: unknown;
  try {
    doc = JSON.parse(contentJson);
  } catch {
    return null;
  }
  let mutated = false;
  const visit = (node: unknown): unknown => {
    if (!node || typeof node !== "object") return node;
    const n = node as { type?: string; attrs?: Record<string, unknown>; content?: unknown[] };
    // taskList был wrapper для taskItem'ов — разворачиваем в плоский список taskRef
    // (taskRef block уровня, не нуждается в контейнере).
    if (n.type === "taskList" && Array.isArray(n.content)) {
      mutated = true;
      return { _flatten: n.content.map(visit) };
    }
    if (n.type === "taskItem" && n.attrs && typeof n.attrs.taskId === "string") {
      mutated = true;
      return { type: "taskRef", attrs: { taskId: n.attrs.taskId } };
    }
    if (Array.isArray(n.content)) {
      // Flatten nested taskList unwrap'ы в parent content.
      const newContent: unknown[] = [];
      for (const child of n.content) {
        const v = visit(child);
        if (v && typeof v === "object" && "_flatten" in (v as object)) {
          for (const f of (v as { _flatten: unknown[] })._flatten) newContent.push(f);
        } else {
          newContent.push(v);
        }
      }
      return { ...n, content: newContent };
    }
    return n;
  };
  const next = visit(doc);
  return mutated ? JSON.stringify(next) : null;
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
  // Pattern B (2026-05-20): Eden больше не диффит taskItem'ы в save flow.
  // task_obj — независимый объект; TaskRef NodeView читает его live из ARK.
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
    const nextNoteType =
      props.noteTypes.find((noteType) => noteType.id === normalizedTypeId) ?? null;
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
    if ((event.metaKey || event.ctrlKey) && event.code === "KeyS") {
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

  // Block selection keyboard: Esc clears, Delete/Backspace удаляет.
  // Active только когда есть selection — guard внутри onWindowKeyDown.
  window.addEventListener("keydown", onWindowKeyDown);

  // Global mousedown listener — clearSelection на ЛЮБОЙ mousedown в окне
  // (capture phase, чтобы отработать до bubble-cancellation от внутренних
  // элементов с `@mousedown.stop` типа task-ref-row). Покрывает кейс
  // когда юзер кликает по чекбоксу/inside TaskRef row — там mousedown
  // не доходит до editor-content-area (.stop на row).
  const handleGlobalMouseDown = () => {
    if (blockSelection.hasSelection.value) {
      blockSelection.clearSelection();
      // Sync обнуляем DecorationSet прямо здесь — Vue watcher на
      // selectedPositions имеет flush:"post" и сработает только в
      // следующем microtask'е, к этому моменту PM уже обработал
      // mousedown и пользователь видит «застывшие» декорации старых
      // блоков параллельно с фокусом на новой задаче.
      if (editor.value) {
        const view = editor.value.view;
        let tr = view.state.tr.setMeta(
          blockSelectionPluginKey,
          buildBlockSelectionDecorations(view.state.doc, new Set()),
        );
        // Дополнительно коллапсим PM TextSelection если она растянулась
        // через несколько блоков (артефакт rubber-band drag'а). NodeView's
        // recomputeRangeSelection ставит `.is-range-selected` для каждого
        // блока который пересекается с sel — multi-block selection делает
        // несколько задач визуально «выделенными» даже после очистки
        // rubber-band decoration.
        const sel = view.state.selection;
        if (!sel.empty) {
          tr = tr.setSelection(TextSelection.create(view.state.doc, sel.from));
        }
        view.dispatch(tr);
      }
      // Помечаем «этот mousedown был clearing click'ом» — onContentMouseDown
      // увидит флаг и не запустит startTracking (иначе clearSelection
      // в capture-фазе делает hasSelection=false к моменту bubble-фазы,
      // и онКонтент видит чистое состояние → создаёт новый drag).
      justClearedSelection = true;
    }
  };
  window.addEventListener("mousedown", handleGlobalMouseDown, { capture: true });

  onCleanup(() => {
    document.removeEventListener("visibilitychange", handleVisibilityChange);
    window.removeEventListener("beforeunload", handleBeforeUnload);
    window.removeEventListener("keydown", onWindowKeyDown);
    window.removeEventListener("mousemove", onWindowMouseMove);
    window.removeEventListener("mousedown", handleGlobalMouseDown, {
      capture: true,
    } as EventListenerOptions);
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
