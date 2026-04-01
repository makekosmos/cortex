<template>
  <div class="editor-wrapper">
    <div class="editor-header">
      <div class="editor-rail editor-header-rail">
        <div class="editor-header-main">
          <TypedHeader
            :active-note-type="activeNoteType"
            :title="title"
            :header-props="headerProps"
            :validation-error="headerValidationError"
            @header-prop-change="handleHeaderPropChange"
          />
          <input v-model="title" class="title-input" placeholder="Заголовок" />
          <div ref="noteTypeMenuRef" class="note-type-inline">
            <button
              class="note-type-trigger"
              data-testid="typed-note-trigger"
              type="button"
              :style="{ '--note-type-accent': activeNoteType?.color ?? 'var(--text-tertiary)' }"
              @click="isNoteTypeMenuOpen = !isNoteTypeMenuOpen"
            >
              {{ activeNoteType?.name ?? "Обычная заметка" }}
            </button>
            <div v-if="isNoteTypeMenuOpen" class="note-type-menu" data-testid="typed-note-menu">
              <button
                :class="['note-type-menu-item', !noteTypeId && 'is-active']"
                type="button"
                @click="handleNoteTypeChange('')"
              >
                Обычная заметка
              </button>
              <button
                v-for="noteType in noteTypes"
                :key="noteType.id"
                :class="['note-type-menu-item', noteType.id === noteTypeId && 'is-active']"
                type="button"
                @click="handleNoteTypeChange(noteType.id)"
              >
                {{ noteType.name }}
              </button>
            </div>
          </div>
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
import { ref, computed, watch, watchEffect, nextTick } from "vue";
import { useEditor, EditorContent, VueRenderer, VueNodeViewRenderer } from "@tiptap/vue-3";
import StarterKit from "@tiptap/starter-kit";
import { Markdown } from "@tiptap/markdown";
import Placeholder from "@tiptap/extension-placeholder";
import CodeBlockLowlight from "@tiptap/extension-code-block-lowlight";
import { all, createLowlight } from "lowlight";
import Typography from "@tiptap/extension-typography";
import type { Editor as TiptapEditor, Range } from "@tiptap/vue-3";
import type { Instance } from "tippy.js";
import tippy from "tippy.js";
import { Wikilink } from "./Wikilink";
import WikilinkList from "./WikilinkList.vue";
import { SlashCommand } from "./SlashCommand";
import SlashCommandList from "./SlashCommandList.vue";
import TypedHeader from "@/components/typed-notes/TypedHeader.vue";
import CodeBlockView from "@/components/CodeBlockView.vue";
import {
  createDefaultHeaderProps,
  parseHeaderTemplate,
  safeParseHeaderProps,
  validateHeaderProps,
} from "@/lib/typedNotes";
import { resolveLanguageId } from "@/lib/codeBlocks";
import "./Editor.css";

const props = defineProps<{
  entry: Entry;
  allEntries: Entry[];
  noteTypes: NoteType[];
  codeToolsSettings: CodeToolsSettings | null;
  onSave: (entry: Entry) => Promise<SaveEntryResult | null>;
  onNavigate: (entryId: string) => void;
}>();

// --- State ---
const title = ref(props.entry.title);
const noteTypeId = ref<string | null>(props.entry.type_id);
const headerLayout = ref<string | null>(props.entry.header_layout);
const headerProps = ref<Record<string, unknown>>(
  safeParseHeaderProps(null, props.entry.header_props_json),
);
const saveConflict = ref<string | null>(null);
const headerValidationError = ref<string | null>(null);
const isNoteTypeMenuOpen = ref(false);
const noteTypeMenuRef = ref<HTMLDivElement | null>(null);
const docVersion = ref(0);

// Non-reactive tracking (mirrors React useRef pattern)
let lastSavedSnapshot = "";
let saveRunId = 0;
let lintRunId = 0;

// Live reference to allEntries for suggestion closures
const allEntriesHolder = { current: props.allEntries };
watch(
  () => props.allEntries,
  (v) => {
    allEntriesHolder.current = v;
  },
  { immediate: true },
);

// --- Computed ---
const activeNoteType = computed(
  () => props.noteTypes.find((nt) => nt.id === noteTypeId.value) ?? null,
);

// --- TipTap setup ---
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
        let popup: Instance[];

        return {
          onStart: (renderProps: any) => {
            component = new VueRenderer(WikilinkList, {
              props: renderProps,
              editor: renderProps.editor,
            });
            if (!renderProps.clientRect) return;
            popup = tippy("body", {
              getReferenceClientRect: renderProps.clientRect,
              appendTo: () => document.body,
              content: component.element,
              showOnCreate: true,
              interactive: true,
              trigger: "manual",
              placement: "bottom-start",
            });
          },
          onUpdate(renderProps: any) {
            component.updateProps(renderProps);
            if (!renderProps.clientRect) return;
            popup[0].setProps({ getReferenceClientRect: renderProps.clientRect });
          },
          onKeyDown(renderProps: any) {
            if (renderProps.event.key === "Escape") {
              popup[0].hide();
              return true;
            }
            return (component.ref as any)?.onKeyDown(renderProps) ?? false;
          },
          onExit() {
            popup[0].destroy();
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
            title: "Заголовок 1",
            description: "Большой заголовок раздела",
            icon: "H1",
            command: ({ editor, range }: { editor: TiptapEditor; range: Range }) =>
              editor.chain().focus().deleteRange(range).setNode("heading", { level: 1 }).run(),
          },
          {
            title: "Заголовок 2",
            description: "Средний заголовок",
            icon: "H2",
            command: ({ editor, range }: { editor: TiptapEditor; range: Range }) =>
              editor.chain().focus().deleteRange(range).setNode("heading", { level: 2 }).run(),
          },
          {
            title: "Текст",
            description: "Обычный абзац",
            icon: "P",
            command: ({ editor, range }: { editor: TiptapEditor; range: Range }) =>
              editor.chain().focus().deleteRange(range).setNode("paragraph").run(),
          },
          {
            title: "Список",
            description: "Маркированный список",
            icon: "•",
            command: ({ editor, range }: { editor: TiptapEditor; range: Range }) =>
              editor.chain().focus().deleteRange(range).toggleBulletList().run(),
          },
          {
            title: "Код",
            description: "Блок кода с подсветкой",
            icon: "{}",
            command: ({ editor, range }: { editor: TiptapEditor; range: Range }) =>
              editor.chain().focus().deleteRange(range).toggleCodeBlock().run(),
          },
          {
            title: "Цитата",
            description: "Блок цитирования",
            icon: '"',
            command: ({ editor, range }: { editor: TiptapEditor; range: Range }) =>
              editor.chain().focus().deleteRange(range).toggleBlockquote().run(),
          },
        ].filter((item) => item.title.toLowerCase().includes(query.toLowerCase())),
      render: () => {
        let component: InstanceType<typeof VueRenderer>;
        let popup: Instance[];

        return {
          onStart: (renderProps: any) => {
            component = new VueRenderer(SlashCommandList, {
              props: renderProps,
              editor: renderProps.editor,
            });
            if (!renderProps.clientRect) return;
            popup = tippy("body", {
              getReferenceClientRect: renderProps.clientRect,
              appendTo: () => document.body,
              content: component.element,
              showOnCreate: true,
              interactive: true,
              trigger: "manual",
              placement: "bottom-start",
            });
          },
          onUpdate(renderProps: any) {
            component.updateProps(renderProps);
            if (!renderProps.clientRect) return;
            popup[0].setProps({ getReferenceClientRect: renderProps.clientRect });
          },
          onKeyDown(renderProps: any) {
            if (renderProps.event.key === "Escape") {
              popup[0].hide();
              return true;
            }
            return (component.ref as any)?.onKeyDown(renderProps) ?? false;
          },
          onExit() {
            popup[0].destroy();
            component.destroy();
          },
        };
      },
    },
  }),
];

const editor = useEditor({
  extensions,
  content: (() => {
    try {
      return props.entry.content_json
        ? JSON.parse(props.entry.content_json)
        : { type: "doc", content: [{ type: "paragraph" }] };
    } catch {
      return { type: "doc", content: [{ type: "paragraph" }] };
    }
  })(),
  autofocus: "end",
  editable: true,
  onUpdate: () => {
    docVersion.value++;
  },
});

// --- Helpers ---

function getCurrentSnapshot(): string | null {
  if (!editor.value) return null;
  return JSON.stringify({
    title: title.value || "Без названия",
    noteTypeId: noteTypeId.value,
    headerLayout: headerLayout.value,
    headerProps: headerProps.value,
    doc: editor.value.getJSON(),
    markdown: editor.value.storage?.markdown?.getMarkdown?.() ?? "",
  });
}

function applyCodeBlockText(nodePos: number, nextCode: string): boolean {
  if (!editor.value) return false;
  const codeBlockNode = editor.value.state.doc.nodeAt(nodePos);
  if (!codeBlockNode) return false;
  const contentFrom = nodePos + 1;
  const contentTo = nodePos + codeBlockNode.nodeSize - 1;
  const tr = editor.value.state.tr;
  if (contentTo > contentFrom) tr.delete(contentFrom, contentTo);
  if (nextCode.length > 0) tr.insertText(nextCode, contentFrom);
  editor.value.view.dispatch(tr);
  return true;
}

async function formatAllCodeBlocks() {
  if (!editor.value || !props.codeToolsSettings) return;
  const blocks: Array<{ nodePos: number; code: string; language: string }> = [];
  editor.value.state.doc.descendants((node, position) => {
    if (node.type.name === "codeBlock") {
      blocks.push({
        nodePos: position,
        code: node.textContent,
        language: resolveLanguageId((node.attrs as { language?: string | null }).language),
      });
    }
    return true;
  });
  for (const block of [...blocks].reverse()) {
    const result = await window.api.formatCodeBlock(block.language, block.code);
    if (result.code !== block.code) applyCodeBlockText(block.nodePos, result.code);
  }
}

async function lintAllCodeBlocks() {
  if (!editor.value || !props.codeToolsSettings) return;
  const runId = ++lintRunId;
  const blocks: Array<{ nodePos: number; code: string; language: string }> = [];
  editor.value.state.doc.descendants((node, position) => {
    if (node.type.name === "codeBlock") {
      blocks.push({
        nodePos: position,
        code: node.textContent,
        language: resolveLanguageId((node.attrs as { language?: string | null }).language),
      });
    }
    return true;
  });
  for (const block of blocks) {
    await window.api.lintCodeBlock(block.language, block.code);
  }
  if (runId !== lintRunId) return;
}

async function save() {
  if (!editor.value) return;

  const headerValidation = validateHeaderProps(activeNoteType.value, headerProps.value);
  if (!headerValidation.success) {
    headerValidationError.value = "Проверьте поля верхушки заметки";
    return;
  }

  headerValidationError.value = null;

  const snapshotBeforeSave = getCurrentSnapshot();
  if (!snapshotBeforeSave || lastSavedSnapshot === snapshotBeforeSave) return;

  const currentSaveRunId = ++saveRunId;

  if (props.codeToolsSettings?.formatOnSave) await formatAllCodeBlocks();
  if (props.codeToolsSettings?.lintTrigger === "on_save") await lintAllCodeBlocks();

  const content_json = JSON.stringify(editor.value.getJSON());

  const saveResult = await props.onSave({
    ...props.entry,
    title: title.value || "Без названия",
    content_json,
    type_id: noteTypeId.value,
    header_layout: headerLayout.value,
    header_props_json: JSON.stringify(headerValidation.data),
    schema_version: 1,
    updated_at: Date.now(),
  });

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
  lastSavedSnapshot = getCurrentSnapshot() ?? snapshotBeforeSave;
}

function handleNoteTypeChange(nextTypeId: string) {
  const nextNoteType = props.noteTypes.find((nt) => nt.id === nextTypeId) ?? null;
  noteTypeId.value = nextTypeId || null;
  headerLayout.value = nextNoteType
    ? parseHeaderTemplate(nextNoteType.header_template_json).kind
    : null;
  headerProps.value = createDefaultHeaderProps(nextNoteType);
  headerValidationError.value = null;
  isNoteTypeMenuOpen.value = false;
}

function handleHeaderPropChange(fieldId: string, value: unknown) {
  headerProps.value = { ...headerProps.value, [fieldId]: value };
}

// --- Watchers ---

// Sync editor content when entry changes externally
watch(
  () => props.entry.content_json,
  (contentJson) => {
    if (!editor.value) return;
    try {
      const parsed = contentJson
        ? JSON.parse(contentJson)
        : { type: "doc", content: [{ type: "paragraph" }] };
      const current = editor.value.getJSON();
      if (JSON.stringify(current) !== JSON.stringify(parsed)) {
        editor.value.commands.setContent(parsed);
      }
    } catch (e) {
      console.error("Failed to parse content_json", e);
    }
  },
);

// Reset state when switching entries
watch(
  () => props.entry.id,
  () => {
    saveConflict.value = null;
  },
);

// Reset snapshot baseline when editor loads or entry changes
watch([editor, () => props.entry.id, () => props.entry.updated_at], () => {
  if (!editor.value) return;
  lastSavedSnapshot = getCurrentSnapshot() ?? "";
});

// Sync header state when entry metadata changes
watch(
  [
    () => props.entry.id,
    () => props.entry.type_id,
    () => props.entry.header_props_json,
    () => props.entry.header_layout,
    () => props.noteTypes,
  ],
  () => {
    noteTypeId.value = props.entry.type_id;
    headerLayout.value = props.entry.header_layout;
    const nextNoteType = props.noteTypes.find((nt) => nt.id === props.entry.type_id) ?? null;
    headerProps.value = safeParseHeaderProps(nextNoteType, props.entry.header_props_json);
    headerValidationError.value = null;
  },
);

// Clear saveConflict on title change
watch(title, () => {
  saveConflict.value = null;
});

// Auto-save: 800ms debounce on doc or title change
watchEffect((onCleanup) => {
  const _d = docVersion.value;
  const _t = title.value;
  const timer = setTimeout(() => {
    void save();
  }, 800);
  onCleanup(() => clearTimeout(timer));
});

// Lint on idle: 1000ms debounce
watchEffect((onCleanup) => {
  if (props.codeToolsSettings?.lintTrigger !== "on_idle") return;
  const _d = docVersion.value;
  const timer = window.setTimeout(() => {
    void lintAllCodeBlocks();
  }, 1000);
  onCleanup(() => window.clearTimeout(timer));
});

// Ctrl+S save handler
watchEffect((onCleanup) => {
  if (!editor.value) return;
  const handleKeyDown = (e: KeyboardEvent) => {
    if ((e.metaKey || e.ctrlKey) && e.key === "s") {
      e.preventDefault();
      void save();
    }
  };
  document.addEventListener("keydown", handleKeyDown);
  onCleanup(() => document.removeEventListener("keydown", handleKeyDown));
});

// Wikilink click navigation
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

// Note type menu close on click-outside / Escape
watchEffect((onCleanup) => {
  if (!isNoteTypeMenuOpen.value) return;

  const handlePointerDown = (event: MouseEvent) => {
    if (noteTypeMenuRef.value?.contains(event.target as Node)) return;
    isNoteTypeMenuOpen.value = false;
  };
  const handleEscape = (event: KeyboardEvent) => {
    if (event.key === "Escape") isNoteTypeMenuOpen.value = false;
  };

  window.addEventListener("mousedown", handlePointerDown);
  window.addEventListener("keydown", handleEscape);
  onCleanup(() => {
    window.removeEventListener("mousedown", handlePointerDown);
    window.removeEventListener("keydown", handleEscape);
  });
});

// Focus editor when entry changes
watch(
  () => props.entry.id,
  () => {
    nextTick(() => editor.value?.commands.focus("end"));
  },
);
</script>
