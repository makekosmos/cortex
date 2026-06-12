<template>
  <div
    :class="[
      'cm-editor-host',
      'kosmos-scroll',
      { 'is-vim-mode': props.vimMode, 'is-focus-mode': props.zenMode },
    ]"
    data-testid="cm-editor-host"
  >
    <div ref="titleContainerRef" class="cm-editor-title-container"></div>
    <div ref="containerRef" class="cm-editor-container"></div>
  </div>
</template>

<script setup lang="ts">
import { onMounted, onBeforeUnmount, ref, watch } from "vue";
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
  onSave: (entry: Entry) => Promise<unknown>;
  zenMode?: boolean;
  vimMode?: boolean;
}

interface Emits {
  closeEntry: [];
  setZenMode: [enabled: boolean];
  entryDraftChange: [entry: Entry];
  liveCharCount: [count: number];
}

const props = defineProps<Props>();
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

const containerRef = ref<HTMLDivElement | null>(null);
const titleContainerRef = ref<HTMLDivElement | null>(null);
const title = ref(getEditableEntryTitle(props.entry.title, props.entry.header_props_json));
const vimCompartment = new Compartment();
const titleVimCompartment = new Compartment();

let view: EditorView | null = null;
let titleView: EditorView | null = null;
let converter: ReturnType<typeof createMdConverter> | null = null;
let autosaveTimer: ReturnType<typeof setTimeout> | null = null;
let lastPersistedTitle = props.entry.title;
let lastPersistedHeaderPropsJson = normalizeHeaderPropsJson(props.entry.header_props_json);

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

function parseHeaderProps(headerPropsJson: string | null | undefined): Record<string, unknown> {
  try {
    return JSON.parse(normalizeHeaderPropsJson(headerPropsJson)) as Record<string, unknown>;
  } catch {
    return {};
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
      parseHeaderProps(props.entry.header_props_json),
      editedTitle,
      lastPersistedTitle,
      lastPersistedHeaderPropsJson,
    ),
  );

  return {
    ...props.entry,
    title: normalizedTitle,
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

function closeEntry(): void {
  emit("closeEntry");
}

function setZenMode(enabled: boolean): void {
  emit("setZenMode", enabled);
}

function createTitleState(initialTitle: string): EditorState {
  return EditorState.create({
    doc: initialTitle,
    extensions: [
      titleVimCompartment.of(props.vimMode ? vim() : []),
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
        if (!update.docChanged) return;
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
        if (update.docChanged) {
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
  view.scrollDOM.classList.add("kosmos-scroll");
});

watch(
  () => props.entry.id,
  () => {
    title.value = getEditableEntryTitle(props.entry.title, props.entry.header_props_json);
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
  overflow-y: auto;
  overflow-x: hidden;
  overscroll-behavior: contain;
}

.cm-editor-title-container {
  width: 100%;
  box-sizing: border-box;
  padding: 16px 32px 4px 32px;
  max-width: 760px;
  margin: 0 auto;
  align-self: stretch;
  flex: 0 0 auto;
}

.cm-editor-host.is-focus-mode .cm-editor-title-container {
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

.cm-editor-title-container :deep(.cm-title-editor .cm-content:has(.cm-line br)::before) {
  content: attr(data-placeholder);
  color: var(--text-secondary);
  position: absolute;
  inset: 0 auto auto 0;
  pointer-events: none;
}

.cm-editor-container {
  flex: 0 0 auto;
  min-height: calc(100% - 56px);
  overflow: visible;
  display: flex;
  flex-direction: column;
}
</style>
