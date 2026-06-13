<template>
  <div
    ref="hostRef"
    :class="[
      'cm-editor-host',
      'kosmos-scroll',
      {
        'is-vim-mode': props.vimMode,
        'is-focus-mode': props.zenMode,
        'is-reader-mode': props.readerMode,
      },
    ]"
    data-testid="cm-editor-host"
  >
    <div ref="titleShellRef" class="cm-editor-title-shell">
      <div
        v-show="showCmTitleEditor"
        ref="titleContainerRef"
        class="cm-editor-title-container"
      ></div>
      <TypedHeader
        v-if="showTypedHeader"
        :active-note-type="activeNoteType"
        :title="getCurrentTitle()"
        :header-props="headerProps"
        :validation-error="null"
        :all-entries="allEntries"
        :current-entry-id="entry.id"
        :note-types="noteTypes"
        :editable-type="true"
        :readonly="false"
        :show-type-row="false"
        :show-title="isPersonEntry"
        @header-prop-change="handleHeaderPropChange"
        @object-type-change="handleTypePick"
        @relation-navigate="props.onNavigate"
      />
    </div>
    <div ref="containerRef" class="cm-editor-container"></div>
  </div>
</template>

<script setup lang="ts">
import { computed, onMounted, onBeforeUnmount, ref, watch } from "vue";
import TypedHeader from "@/components/typed-notes/TypedHeader.vue";
import { Compartment, EditorState, Prec } from "@codemirror/state";
import { drawSelection, EditorView, keymap } from "@codemirror/view";
import { history, historyKeymap, defaultKeymap } from "@codemirror/commands";
import { highlightActiveLine } from "@codemirror/view";
import { markdown, markdownLanguage } from "@codemirror/lang-markdown";
import { syntaxHighlighting, defaultHighlightStyle } from "@codemirror/language";
import { autocompletion, completionKeymap } from "@codemirror/autocomplete";
import { createMdConverter } from "./mdConvert";
import {
  getEditableEntryTitle,
  resolveStoredEntryTitle,
  syncUntitledEntryTitleFlag,
} from "@/lib/entryTitles";
import {
  createHeaderPropsForTypeChange,
  resolveNoteTypeHeaderLayout,
  safeParseHeaderProps,
} from "@/lib/typedNotes";
import { SYSTEM_TYPE_NOTE_ID, SYSTEM_TYPE_PERSON_ID } from "@/lib/systemTypes";
import { livePreviewPlugin } from "./cm/live-preview";
import { codeBlockFontPlugin } from "./cm/code-block-font";
import { fatCursorFixPlugin } from "./cm/fat-cursor-fix";
import { markdownListIndentPlugin } from "./cm/list-indent";
import { orderedListRenumber } from "./cm/ordered-list-renumber";
import { slashCommandSource } from "./cm/slash-commands";
import { resolveCodeLanguage } from "./cm/code-languages";
import { edenHighlight } from "./cm/highlight";
import { Vim, vim } from "@replit/codemirror-vim";
import "./cm-editor.css";

interface Props {
  entry: Entry;
  allEntries?: Entry[];
  noteTypes?: NoteType[];
  onSave: (entry: Entry) => Promise<unknown>;
  onNavigate?: (entryId: string) => void;
  zenMode?: boolean;
  vimMode?: boolean;
  readerMode?: boolean;
}

interface Emits {
  closeEntry: [];
  setZenMode: [enabled: boolean];
  entryDraftChange: [entry: Entry];
  liveCharCount: [count: number];
  titleOutOfViewChange: [outOfView: boolean];
  typeChange: [entry: Entry];
}

const props = withDefaults(defineProps<Props>(), {
  allEntries: () => [],
  noteTypes: () => [],
  onNavigate: () => {},
  zenMode: false,
  vimMode: false,
  readerMode: false,
});
const emit = defineEmits<Emits>();

const AUTOSAVE_DEBOUNCE_MS = 300;
type EdenVimActions = {
  save: () => Promise<void>;
  close: () => void;
  setZenMode: (enabled: boolean) => void;
  getZenMode: () => boolean;
};

let vimCommandsRegistered = false;
let activeVimActions: EdenVimActions | null = null;

function registerEdenVimCommands(): void {
  if (vimCommandsRegistered) return;
  vimCommandsRegistered = true;

  Vim.defineEx("write", "w", () => {
    void activeVimActions?.save().catch((err) => {
      console.warn("[eden cm] :w failed:", err);
    });
  });
  Vim.defineEx("quit", "q", () => {
    activeVimActions?.close();
  });
  Vim.defineEx("wq", "wq", () => {
    const actions = activeVimActions;
    if (!actions) return;
    void actions
      .save()
      .then(() => actions.close())
      .catch((err) => {
        console.warn("[eden cm] :wq failed:", err);
      });
  });
  Vim.defineEx("zen", "zen", (_cm: unknown, params: { argString?: string } | undefined) => {
    const mode = (params?.argString ?? "toggle").trim().toLowerCase();
    if (mode === "off") {
      return;
    }
    activeVimActions?.setZenMode(true);
  });
  Vim.defineEx("zenmode", "zenmode", () => {
    activeVimActions?.setZenMode(true);
  });
}

const hostRef = ref<HTMLDivElement | null>(null);
const containerRef = ref<HTMLDivElement | null>(null);
const titleContainerRef = ref<HTMLDivElement | null>(null);
const titleShellRef = ref<HTMLDivElement | null>(null);
const title = ref(getEditableEntryTitle(props.entry.title, props.entry.header_props_json));
const headerProps = ref<Record<string, unknown>>(
  safeParseHeaderProps(
    props.noteTypes.find(
      (noteType) => noteType.id === (props.entry.type_id ?? SYSTEM_TYPE_NOTE_ID),
    ) ?? null,
    props.entry.header_props_json,
  ),
);
const headerLayout = ref<string | null>(props.entry.header_layout);
const vimCompartment = new Compartment();
const titleVimCompartment = new Compartment();
const editableCompartment = new Compartment();
const titleEditableCompartment = new Compartment();

let view: EditorView | null = null;
let titleView: EditorView | null = null;
let converter: ReturnType<typeof createMdConverter> | null = null;
let autosaveTimer: ReturnType<typeof setTimeout> | null = null;
let bodyScrollElement: HTMLElement | null = null;
let titleOutOfView = false;
let lastPersistedTitle = props.entry.title;
let lastPersistedHeaderPropsJson = normalizeHeaderPropsJson(props.entry.header_props_json);

const currentTypeId = ref(props.entry.type_id ?? SYSTEM_TYPE_NOTE_ID);
const activeNoteType = computed(
  () => props.noteTypes.find((noteType) => noteType.id === currentTypeId.value) ?? null,
);
const isPersonEntry = computed(() => currentTypeId.value === SYSTEM_TYPE_PERSON_ID);
const showTypedHeader = computed(() => Boolean(activeNoteType.value));
const showCmTitleEditor = computed(() => !isPersonEntry.value);
function getCurrentTitle(): string {
  return titleView?.state.doc.toString() ?? title.value;
}

function normalizeHeaderPropsJson(headerPropsJson: string | null | undefined): string {
  if (!headerPropsJson?.trim()) {
    return JSON.stringify({});
  }

  try {
    return JSON.stringify(JSON.parse(headerPropsJson) as Record<string, unknown>);
  } catch {
    return JSON.stringify({});
  }
}

function buildEntryDraft(contentJson?: string): Entry {
  const editedTitle = getCurrentTitle();
  const normalizedTitle = resolveStoredEntryTitle(
    editedTitle,
    lastPersistedTitle,
    lastPersistedHeaderPropsJson,
  );
  const normalizedHeaderPropsJson = JSON.stringify(
    syncUntitledEntryTitleFlag(
      headerProps.value,
      editedTitle,
      lastPersistedTitle,
      lastPersistedHeaderPropsJson,
    ),
  );

  return {
    ...props.entry,
    title: normalizedTitle,
    type_id: currentTypeId.value,
    header_layout: headerLayout.value,
    header_props_json: normalizedHeaderPropsJson,
    content_json: contentJson ?? props.entry.content_json,
    updated_at: Date.now(),
  };
}

function scheduleAutosave(): void {
  if (autosaveTimer !== null) {
    clearTimeout(autosaveTimer);
  }
  autosaveTimer = setTimeout(() => {
    autosaveTimer = null;
    void flushSave().catch((err) => {
      console.warn("[eden cm] autosave failed:", err);
    });
  }, AUTOSAVE_DEBOUNCE_MS);
}

async function flushSave(): Promise<void> {
  if (!view || !converter) return;
  const md = view.state.doc.toString();
  const contentJson = JSON.stringify(converter.markdownToJson(md));
  const entry = buildEntryDraft(contentJson);
  emit("entryDraftChange", entry);
  await props.onSave(entry);
  lastPersistedTitle = entry.title;
  lastPersistedHeaderPropsJson = normalizeHeaderPropsJson(entry.header_props_json);
}

async function handleTypePick(nextTypeId: string): Promise<void> {
  if (!view || !converter) return;

  const nextType = props.noteTypes.find((noteType) => noteType.id === nextTypeId) ?? null;
  if (!nextType || nextTypeId === currentTypeId.value) return;

  const md = view.state.doc.toString();
  const contentJson = JSON.stringify(converter.markdownToJson(md));
  const nextHeaderProps = createHeaderPropsForTypeChange(nextType, getCurrentTitle());
  currentTypeId.value = nextTypeId;
  headerProps.value = nextHeaderProps;
  headerLayout.value = resolveNoteTypeHeaderLayout(nextType);
  const entry: Entry = {
    ...buildEntryDraft(contentJson),
    type_id: nextTypeId,
    header_layout: headerLayout.value,
    header_props_json: JSON.stringify(nextHeaderProps),
  };

  emit("typeChange", entry);
  emit("entryDraftChange", entry);

  void window.api?.saveNoteType?.(nextType).catch((err) => {
    console.warn("[eden cm] type persist failed:", err);
  });

  const result = await props.onSave(entry);
  if (result && typeof result === "object" && "ok" in result && result.ok === false) {
    console.warn("[eden cm] type change save failed:", result);
  }
  lastPersistedTitle = entry.title;
  lastPersistedHeaderPropsJson = normalizeHeaderPropsJson(entry.header_props_json);
}

function handleHeaderPropChange(fieldId: string, value: unknown): void {
  headerProps.value = { ...headerProps.value, [fieldId]: value };
  emit("entryDraftChange", buildEntryDraft());
  scheduleAutosave();
}

function shouldBackfillPersonName(noteType: NoteType | null): boolean {
  if (noteType?.id !== SYSTEM_TYPE_PERSON_ID) return false;

  const hasPersonName = ["first_name", "last_name", "patronymic"].some((fieldId) =>
    String(headerProps.value[fieldId] ?? "").trim(),
  );

  return !hasPersonName && getCurrentTitle().trim().length > 0;
}

function backfillPersonNameFromTitle(noteType: NoteType | null): void {
  if (!shouldBackfillPersonName(noteType)) return;

  headerProps.value = {
    ...headerProps.value,
    ...createHeaderPropsForTypeChange(noteType, getCurrentTitle()),
  };
  emit("entryDraftChange", buildEntryDraft());
  scheduleAutosave();
}

function closeEntry(): void {
  emit("closeEntry");
}

function setZenMode(enabled: boolean): void {
  emit("setZenMode", enabled);
}

function syncTitleScrollState(): void {
  const titleHeight = isPersonEntry.value
    ? (titleShellRef.value?.offsetHeight ?? 0)
    : (titleContainerRef.value?.offsetHeight ?? 0);
  const scrollTop = bodyScrollElement?.scrollTop ?? 0;
  const nextOutOfView = titleHeight > 0 && scrollTop >= titleHeight;
  if (nextOutOfView === titleOutOfView) return;

  titleOutOfView = nextOutOfView;
  emit("titleOutOfViewChange", nextOutOfView);
}

function createTitleState(initialTitle: string): EditorState {
  return EditorState.create({
    doc: initialTitle,
    extensions: [
      titleVimCompartment.of(props.vimMode ? vim() : []),
      titleEditableCompartment.of(EditorView.editable.of(!props.readerMode)),
      drawSelection(),
      EditorView.lineWrapping,
      fatCursorFixPlugin,
      EditorView.editorAttributes.of({ class: "cm-title-editor" }),
      EditorView.contentAttributes.of({
        spellcheck: "false",
        "aria-label": "Название заметки",
        "data-placeholder": "Без названия",
      }),
      Prec.highest(
        keymap.of([
          {
            key: "Enter",
            run: () => {
              view?.focus();
              return true;
            },
          },
        ]),
      ),
      EditorView.domEventHandlers({
        blur: () => {
          void flushSave().catch((err) => {
            console.warn("[eden cm] title blur save failed:", err);
          });
        },
      }),
      EditorView.updateListener.of((update) => {
        if (!update.docChanged || props.readerMode) return;
        const nextTitle = update.state.doc.toString().replace(/[\r\n]+/g, " ");
        if (nextTitle !== update.state.doc.toString()) {
          update.view.dispatch({
            changes: { from: 0, to: update.state.doc.length, insert: nextTitle },
          });
          return;
        }

        title.value = nextTitle;
        emit("entryDraftChange", buildEntryDraft());
        scheduleAutosave();
      }),
    ],
  });
}

onMounted(() => {
  if (!containerRef.value || !titleContainerRef.value) return;

  registerEdenVimCommands();
  activeVimActions = {
    save: flushSave,
    close: closeEntry,
    setZenMode,
    getZenMode: () => !!props.zenMode,
  };

  converter = createMdConverter();

  let initialMd = "";
  try {
    const doc = JSON.parse(props.entry.content_json) as object;
    initialMd = converter.jsonToMarkdown(doc);
  } catch {
    initialMd = "";
  }

  titleView = new EditorView({
    state: createTitleState(title.value),
    parent: titleContainerRef.value,
  });

  const state = EditorState.create({
    doc: initialMd,
    extensions: [
      vimCompartment.of(props.vimMode ? vim() : []),
      editableCompartment.of(EditorView.editable.of(!props.readerMode)),
      history(),
      drawSelection(),
      highlightActiveLine(),
      EditorView.lineWrapping,
      markdown({ base: markdownLanguage, codeLanguages: resolveCodeLanguage, addKeymap: true }),
      syntaxHighlighting(edenHighlight),
      syntaxHighlighting(defaultHighlightStyle, { fallback: true }),
      livePreviewPlugin,
      codeBlockFontPlugin,
      fatCursorFixPlugin,
      markdownListIndentPlugin,
      orderedListRenumber,
      autocompletion({ override: [slashCommandSource] }),
      keymap.of([...defaultKeymap, ...historyKeymap, ...completionKeymap]),
      EditorView.contentAttributes.of({ spellcheck: "false" }),
      EditorView.updateListener.of((update) => {
        if (update.docChanged && !props.readerMode) {
          emit("liveCharCount", update.state.doc.length);
          scheduleAutosave();
        }
      }),
    ],
  });

  view = new EditorView({
    state,
    parent: containerRef.value,
  });
  bodyScrollElement = hostRef.value;
  bodyScrollElement.classList.add("kosmos-scroll");
  bodyScrollElement.addEventListener("scroll", syncTitleScrollState, { passive: true });
  backfillPersonNameFromTitle(activeNoteType.value);
  syncTitleScrollState();
});

watch(
  () => props.entry.id,
  () => {
    const normalizedTypeId = props.entry.type_id ?? SYSTEM_TYPE_NOTE_ID;
    const noteType = props.noteTypes.find((candidate) => candidate.id === normalizedTypeId) ?? null;
    currentTypeId.value = normalizedTypeId;
    title.value = getEditableEntryTitle(props.entry.title, props.entry.header_props_json);
    headerProps.value = safeParseHeaderProps(noteType, props.entry.header_props_json);
    headerLayout.value = props.entry.header_layout ?? resolveNoteTypeHeaderLayout(noteType);
    backfillPersonNameFromTitle(noteType);
    titleOutOfView = false;
    emit("titleOutOfViewChange", false);
    if (titleView && titleView.state.doc.toString() !== title.value) {
      titleView.dispatch({
        changes: { from: 0, to: titleView.state.doc.length, insert: title.value },
      });
    }
    lastPersistedTitle = props.entry.title;
    lastPersistedHeaderPropsJson = normalizeHeaderPropsJson(props.entry.header_props_json);
  },
);

watch(
  [
    () => props.entry.type_id,
    () => props.entry.header_layout,
    () => props.entry.header_props_json,
    () => props.noteTypes,
  ],
  () => {
    const normalizedTypeId = props.entry.type_id ?? SYSTEM_TYPE_NOTE_ID;
    const noteType = props.noteTypes.find((candidate) => candidate.id === normalizedTypeId) ?? null;
    currentTypeId.value = normalizedTypeId;
    headerProps.value = safeParseHeaderProps(noteType, props.entry.header_props_json);
    headerLayout.value = props.entry.header_layout ?? resolveNoteTypeHeaderLayout(noteType);
    backfillPersonNameFromTitle(noteType);
  },
);

watch(
  () => props.readerMode,
  (enabled) => {
    const extension = EditorView.editable.of(!enabled);
    view?.dispatch({
      effects: [editableCompartment.reconfigure(extension)],
    });
    titleView?.dispatch({
      effects: [titleEditableCompartment.reconfigure(extension)],
    });
  },
);

watch(
  () => props.vimMode,
  (enabled) => {
    const extension = enabled ? vim() : [];
    view?.dispatch({
      effects: [vimCompartment.reconfigure(extension)],
    });
    titleView?.dispatch({
      effects: [titleVimCompartment.reconfigure(extension)],
    });
  },
);

onBeforeUnmount(() => {
  if (activeVimActions?.save === flushSave) {
    activeVimActions = null;
  }

  if (autosaveTimer !== null) {
    clearTimeout(autosaveTimer);
    autosaveTimer = null;
  }

  const unmountSave = flushSave();
  bodyScrollElement?.removeEventListener("scroll", syncTitleScrollState);
  bodyScrollElement = null;
  titleView?.destroy();
  titleView = null;
  view?.destroy();
  view = null;
  converter?.destroy();
  converter = null;

  void unmountSave.catch((err) => {
    console.warn("[eden cm] unmount save failed:", err);
  });
});
</script>

<style scoped>
.cm-editor-host {
  width: 100%;
  height: 100%;
  display: flex;
  flex-direction: column;
  overflow: auto;
  overflow-x: hidden;
  overscroll-behavior: contain;
}

.cm-editor-title-shell {
  width: 100%;
  box-sizing: border-box;
  padding: 18px 32px 8px 32px;
  max-width: 760px;
  margin: 0 auto;
  align-self: stretch;
  flex: 0 0 auto;
  display: grid;
  gap: 10px;
}

.cm-editor-title-container {
  width: 100%;
  min-width: 0;
}

.cm-editor-host.is-focus-mode .cm-editor-title-shell {
  display: none;
}

.cm-editor-title-container :deep(.cm-title-editor) {
  background: transparent;
  color: var(--text-primary);
  font-family: var(--font-sans);
  font-size: 1.5em;
  font-weight: 700;
  outline: none;
}

.cm-editor-title-container :deep(.cm-title-editor .cm-scroller) {
  overflow: visible;
  font-family: inherit;
  line-height: 1.25;
  min-height: 0;
  padding: 0;
}

.cm-editor-title-container :deep(.cm-title-editor .cm-content) {
  padding: 0;
  caret-color: var(--eden-accent-color, var(--accent, currentColor));
  font-family: inherit;
  line-height: 1.25;
  min-height: 0;
  position: relative;
}

.cm-editor-title-container :deep(.cm-title-editor .cm-line) {
  padding: 0;
  font-family: inherit;
  line-height: 1.25;
  min-height: 0;
}

.cm-editor-host.is-vim-mode .cm-editor-title-container :deep(.cm-title-editor),
.cm-editor-host.is-vim-mode .cm-editor-title-container :deep(.cm-title-editor .cm-line) {
  font-family: var(--font-mono);
  font-weight: 400;
}

.cm-editor-host.is-reader-mode,
.cm-editor-host.is-reader-mode :deep(.cm-editor),
.cm-editor-host.is-reader-mode :deep(.cm-content),
.cm-editor-host.is-reader-mode :deep(.cm-line),
.cm-editor-host.is-reader-mode .cm-editor-title-container :deep(.cm-title-editor),
.cm-editor-host.is-reader-mode .cm-editor-title-container :deep(.cm-title-editor .cm-line) {
  font-family: "SF Pro Text", "SF Pro Display", Inter, var(--font-sans), system-ui, sans-serif;
}

.cm-editor-host.is-reader-mode :deep(.cm-content) {
  cursor: default;
}

.cm-editor-host.is-reader-mode :deep(.cm-cursor),
.cm-editor-host.is-reader-mode :deep(.cm-dropCursor) {
  display: none;
}

.cm-editor-title-container :deep(.cm-title-editor .cm-content:has(.cm-line br)::before) {
  content: attr(data-placeholder);
  color: var(--text-secondary);
  position: absolute;
  inset: 0 auto auto 0;
  pointer-events: none;
}

.cm-editor-title-shell :deep(.typed-object-header) {
  margin: 2px 0 8px;
  overflow: visible;
}

.cm-editor-title-shell :deep(.typed-object-header__inner) {
  gap: 10px;
  padding: 0;
}

.cm-editor-title-shell :deep(.typed-object-header.is-person .typed-object-header__hero) {
  gap: 12px;
}

.cm-editor-title-shell :deep(.typed-object-header.is-person .typed-object-header__content) {
  gap: 8px;
}

.cm-editor-title-shell :deep(.typed-object-header.is-person .typed-object-header__title) {
  max-width: 100%;
  color: var(--text-primary);
  font-family: "SF Pro Text", "SF Pro Display", Inter, var(--font-sans), system-ui, sans-serif;
  font-size: 28px;
  line-height: 32px;
  font-weight: 700;
  letter-spacing: -0.56px;
  text-wrap: balance;
}

.cm-editor-title-shell :deep(.typed-object-header.is-person .typed-object-header__featured--column),
.cm-editor-title-shell :deep(.typed-object-header.is-person .typed-object-header__secondary-list) {
  width: min(100%, 560px);
}

.cm-editor-title-shell :deep(.typed-object-header__featured--column) {
  gap: 6px;
  padding-top: 4px;
}

.cm-editor-title-shell :deep(.typed-object-header__secondary) {
  padding-top: 0;
}

.cm-editor-title-shell :deep(.object-property-field--featured-column),
.cm-editor-title-shell :deep(.object-property-field--secondary) {
  grid-template-columns: minmax(116px, 30%) minmax(0, 1fr);
  gap: 12px;
  min-height: 34px;
  padding: 3px 0;
}

.cm-editor-title-shell :deep(.object-property-field--secondary:hover) {
  background: transparent;
}

.cm-editor-title-shell :deep(.object-property-field__label),
.cm-editor-title-shell :deep(.object-property-field__value),
.cm-editor-title-shell :deep(.object-property-field__input),
.cm-editor-title-shell :deep(.object-property-picker__trigger),
.cm-editor-title-shell :deep(.object-property-picker__summary),
.cm-editor-title-shell :deep(.object-property-picker__placeholder) {
  font-family:
    "SF Pro Text", "SF Pro Display", Inter, var(--font-sans), system-ui, sans-serif !important;
  font-size: 14px !important;
  font-weight: 400 !important;
  line-height: 22px !important;
  letter-spacing: -0.12px !important;
}

.cm-editor-title-shell :deep(.object-property-field__input) {
  height: 30px;
  min-height: 30px;
  border-radius: 8px;
  background: transparent;
  caret-color: var(--eden-accent-color, var(--accent, currentColor));
}

.cm-editor-title-shell :deep(.object-property-field__input::placeholder) {
  font: inherit;
  color: var(--muted-foreground);
  opacity: 1;
}

.cm-editor-title-shell :deep(.object-property-picker__trigger) {
  height: 30px;
  min-height: 30px;
  padding: 0;
  background: transparent;
}

.cm-editor-title-shell :deep(.typed-object-header__avatar-picker .object-property-picker__trigger) {
  width: 128px;
  height: 128px;
  min-height: 128px;
  place-items: center;
}

.cm-editor-title-shell
  :deep(.typed-object-header__avatar-picker .object-property-picker__placeholder) {
  display: block;
  width: 100%;
  text-align: center;
  font-size: 30px;
  line-height: 1;
}

.cm-editor-container {
  flex: 0 0 auto;
  min-height: 0;
  overflow: visible;
  display: flex;
  flex-direction: column;
}

.cm-editor-container :deep(.cm-editor) {
  height: auto;
  min-height: 0;
  contain: none;
}

.cm-editor-container :deep(.cm-scroller) {
  overflow: visible;
  min-height: 0;
}
</style>
