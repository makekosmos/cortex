<template>
  <div
    :class="['cm-editor-host', 'kosmos-scroll', { 'is-vim-mode': props.vimMode }]"
    data-testid="cm-editor-host"
  >
    <input
      ref="titleInputRef"
      class="cm-editor-title-input"
      type="text"
      placeholder="Без названия"
      :value="props.entry.title"
      @blur="onTitleBlur"
    />
    <div ref="containerRef" class="cm-editor-container"></div>
  </div>
</template>

<script setup lang="ts">
import { onMounted, onBeforeUnmount, ref, watch } from "vue";
import { Compartment } from "@codemirror/state";
import { drawSelection, EditorView, keymap } from "@codemirror/view";
import { history, historyKeymap, defaultKeymap } from "@codemirror/commands";
import { highlightActiveLine } from "@codemirror/view";
import { markdown, markdownLanguage } from "@codemirror/lang-markdown";
import { syntaxHighlighting, defaultHighlightStyle } from "@codemirror/language";
import { autocompletion, completionKeymap } from "@codemirror/autocomplete";
import { createMdConverter } from "./mdConvert";
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
  exitZen: [];
  closeEntry: [];
  setZenMode: [enabled: boolean];
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
    if (mode === "on") {
      activeVimActions?.setZenMode(true);
      return;
    }
    if (mode === "off") {
      activeVimActions?.setZenMode(false);
      return;
    }
    activeVimActions?.setZenMode(!(activeVimActions?.getZenMode() ?? false));
  });
  Vim.defineEx("zenmode", "zenmode", () => {
    activeVimActions?.setZenMode(!(activeVimActions?.getZenMode() ?? false));
  });
}

const containerRef = ref<HTMLDivElement | null>(null);
const titleInputRef = ref<HTMLInputElement | null>(null);
const vimCompartment = new Compartment();

let view: EditorView | null = null;
let converter: ReturnType<typeof createMdConverter> | null = null;
let autosaveTimer: ReturnType<typeof setTimeout> | null = null;

function getCurrentTitle(): string {
  return titleInputRef.value?.value ?? props.entry.title;
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
  const entry: Entry = {
    ...props.entry,
    title: getCurrentTitle(),
    content_json: contentJson,
    updated_at: Date.now(),
  };
  await props.onSave(entry);
}

function closeEntry(): void {
  emit("closeEntry");
}

function setZenMode(enabled: boolean): void {
  emit("setZenMode", enabled);
}

function onKeydown(e: KeyboardEvent): void {
  if (e.key === "Escape" && props.zenMode) {
    emit("exitZen");
  }
}

function onTitleBlur(): void {
  void flushSave().catch((err) => {
    console.warn("[eden cm] title blur save failed:", err);
  });
}

onMounted(() => {
  if (!containerRef.value) return;

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

  view = new EditorView({
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
    parent: containerRef.value,
  });
  view.scrollDOM.classList.add("kosmos-scroll");

  document.addEventListener("keydown", onKeydown);
});

watch(
  () => props.vimMode,
  (enabled) => {
    if (!view) return;
    view.dispatch({
      effects: [vimCompartment.reconfigure(enabled ? vim() : [])],
    });
  },
);

onBeforeUnmount(() => {
  document.removeEventListener("keydown", onKeydown);

  if (activeVimActions?.save === flushSave) {
    activeVimActions = null;
  }

  if (autosaveTimer !== null) {
    clearTimeout(autosaveTimer);
    autosaveTimer = null;
  }

  const unmountSave = flushSave();
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

.cm-editor-title-input {
  width: 100%;
  box-sizing: border-box;
  background: transparent;
  border: none;
  outline: none;
  font-family: var(--font-sans);
  font-size: 1.5em;
  font-weight: 700;
  color: var(--text-primary);
  caret-color: var(--eden-accent-color, var(--accent, currentColor));
  padding: 24px 32px 8px 32px;
  max-width: 760px;
  margin: 0 auto;
  align-self: stretch;
}

.cm-editor-title-input::placeholder {
  color: var(--text-secondary);
}

.cm-editor-container {
  flex: 0 0 auto;
  min-height: calc(100% - 72px);
  overflow: visible;
  display: flex;
  flex-direction: column;
}
</style>
