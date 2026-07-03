<script setup lang="ts">
import {
  computed,
  createApp,
  h,
  nextTick,
  onBeforeUnmount,
  onMounted,
  ref,
  shallowRef,
  type App as VueApp,
  watch,
} from "vue";
import {
  type Editor,
  Extension,
  mergeAttributes,
  Node,
  textblockTypeInputRule,
  wrappingInputRule,
} from "@tiptap/core";
import type { Node as ProseMirrorNode } from "@tiptap/pm/model";
import { Plugin, PluginKey } from "@tiptap/pm/state";
import { Decoration, DecorationSet, type EditorView, type NodeView } from "@tiptap/pm/view";
import { EditorContent, useEditor } from "@tiptap/vue-3";
import StarterKit from "@tiptap/starter-kit";
import TaskItem from "@tiptap/extension-task-item";
import TaskList from "@tiptap/extension-task-list";
import Placeholder from "@tiptap/extension-placeholder";
import {
  CheckSquare,
  Code,
  Heading1,
  Heading2,
  Heading3,
  Heading4,
  Heading5,
  Heading6,
  List,
  ListOrdered,
  Pilcrow,
  Minus,
  Quote,
} from "@lucide/vue";
import {
  PhArrowBendDownRight,
  PhArrowsLeftRight,
  PhCaretDown,
  PhCaretUp,
  PhCheck,
  PhCopy,
} from "@phosphor-icons/vue";
import { Dropdown, Skeleton } from "@kosmos/visuals";
import TypedHeader from "@/components/typed-notes/TypedHeader.vue";
import {
  getEditableEntryTitle,
  resolveStoredEntryTitle,
  syncUntitledEntryTitleFlag,
} from "@/lib/entryTitles";
import { resolveNoteTypeHeaderLayout } from "@/lib/typedNotes";
import { createHeaderPropsForTypeChange, safeParseHeaderProps } from "@/lib/typedNoteHeaderProps";
import { SYSTEM_TYPE_NOTE_ID, SYSTEM_TYPE_PERSON_ID } from "@/lib/systemTypes";
import { isFailedSaveResult } from "@/lib/saveResult";
import {
  isReadableEntryContent,
  markdownToTiptapDoc,
  readEntryTiptapDoc,
  tiptapDocToMarkdown,
  writeEntryTiptapDoc,
  type TiptapDoc,
} from "@/editor-content/content";
import {
  highlightCodeWithShiki,
  isShikiCodeLanguage,
  type ShikiCodeLanguage,
} from "./shikiHighlight";

interface Props {
  entry: Entry;
  allEntries?: Entry[];
  noteTypes?: NoteType[];
  onSave: (entry: Entry) => Promise<unknown>;
  onNavigate?: (entryId: string) => void;
  zenMode?: boolean;
  readerMode?: boolean;
  bodyLoading?: boolean;
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
  readerMode: false,
  bodyLoading: false,
});

const EdenImage = Node.create({
  name: "image",
  group: "block",
  atom: true,
  draggable: true,

  addAttributes() {
    return {
      src: { default: null },
      alt: { default: null },
      title: { default: null },
    };
  },

  parseHTML() {
    return [{ tag: "img[src]" }];
  },

  renderHTML({ HTMLAttributes }) {
    return ["img", mergeAttributes(HTMLAttributes)];
  },
});

const EdenCodeBlockTools = Extension.create({
  name: "edenCodeBlockTools",

  addProseMirrorPlugins() {
    return [
      new Plugin<DecorationSet>({
        key: CODE_BLOCK_PLUGIN_KEY,
        state: {
          init: () => DecorationSet.empty,
          apply(tr, value) {
            const next = tr.getMeta(CODE_BLOCK_PLUGIN_KEY) as DecorationSet | undefined;
            if (next) return next;
            return tr.docChanged ? value.map(tr.mapping, tr.doc) : value;
          },
        },
        props: {
          nodeViews: {
            codeBlock: (node, view, getPos) => new EdenCodeBlockView(node, view, getPos),
          },
          decorations(state): DecorationSet {
            return CODE_BLOCK_PLUGIN_KEY.getState(state) ?? DecorationSet.empty;
          },
        },
        view(editorView) {
          let requestId = 0;
          let lastDoc: ProseMirrorNode | null = null;

          const scheduleHighlight = (): void => {
            const doc = editorView.state.doc;
            if (doc === lastDoc) return;
            lastDoc = doc;
            const currentRequest = ++requestId;

            void buildCodeBlockDecorations(doc).then((decorations) => {
              if (currentRequest !== requestId) return;
              editorView.dispatch(editorView.state.tr.setMeta(CODE_BLOCK_PLUGIN_KEY, decorations));
            });
          };

          scheduleHighlight();
          return {
            update(view, previousState) {
              if (view.state.doc.eq(previousState.doc)) return;
              scheduleHighlight();
            },
            destroy() {
              requestId++;
            },
          };
        },
      }),
    ];
  },
});

class EdenCodeBlockView implements NodeView {
  dom: HTMLElement;
  contentDOM: HTMLElement;

  private static readonly COLLAPSED_LINE_LIMIT = 18;

  private node: ProseMirrorNode;
  private readonly view: EditorView;
  private readonly getPos: (() => number | undefined) | boolean;
  private readonly pre: HTMLPreElement;
  private readonly languageDropdownHost: HTMLElement;
  private readonly languageDropdownValue = shallowRef("");
  private readonly languageDropdownApp: VueApp;
  private readonly wrapButton: HTMLButtonElement;
  private readonly wrapLines = shallowRef(true);
  private readonly wrapIconApp: VueApp;
  private readonly copyButton: HTMLButtonElement;
  private readonly copyIconCopied = shallowRef(false);
  private readonly copyIconApp: VueApp;
  private readonly expandButton: HTMLButtonElement;
  private readonly expandIconApp: VueApp;
  private readonly resizeObserver: ResizeObserver | null = null;
  private scrollTimer: ReturnType<typeof setTimeout> | null = null;
  private copyTimer: ReturnType<typeof setTimeout> | null = null;
  private expanded = false;

  constructor(
    node: ProseMirrorNode,
    view: EditorView,
    getPos: (() => number | undefined) | boolean,
  ) {
    this.node = node;
    this.view = view;
    this.getPos = getPos;

    this.dom = document.createElement("div");
    this.dom.className = "tiptap-code-block is-wrapped";
    this.dom.dataset.language = visibleCodeLanguage(node);

    const toolbar = document.createElement("div");
    toolbar.className = "tiptap-code-toolbar";
    toolbar.contentEditable = "false";

    this.languageDropdownHost = document.createElement("div");
    this.languageDropdownHost.className = "tiptap-code-language-host";
    this.languageDropdownApp = createApp({
      setup: () => () =>
        h(Dropdown, {
          modelValue: this.languageDropdownValue.value,
          options: codeBlockLanguageOptions(this.languageDropdownValue.value),
          placeholder: "Plain text",
          searchable: true,
          searchPlaceholder: "Поиск языка...",
          maxHeightPx: 260,
          matchTriggerWidth: false,
          panelAlign: "end",
          class: "tiptap-code-language-dropdown",
          "onUpdate:modelValue": this.setLanguage,
        }),
    });
    this.languageDropdownApp.mount(this.languageDropdownHost);

    this.wrapButton = document.createElement("button");
    this.wrapButton.type = "button";
    this.wrapButton.className = "tiptap-code-action tiptap-code-wrap";
    this.wrapIconApp = createApp({
      setup: () => () =>
        h(this.wrapLines.value ? PhArrowsLeftRight : PhArrowBendDownRight, {
          size: 15,
          weight: "regular",
          "aria-hidden": "true",
        }),
    });
    this.wrapIconApp.mount(this.wrapButton);
    this.wrapButton.addEventListener("click", this.handleWrapToggle);

    this.copyButton = document.createElement("button");
    this.copyButton.type = "button";
    this.copyButton.className = "tiptap-code-action tiptap-code-copy";
    this.copyButton.title = "Скопировать код";
    this.copyButton.ariaLabel = "Скопировать код";
    this.copyIconApp = createApp({
      setup: () => () =>
        h(this.copyIconCopied.value ? PhCheck : PhCopy, {
          size: 15,
          weight: "regular",
          "aria-hidden": "true",
        }),
    });
    this.copyIconApp.mount(this.copyButton);
    this.copyButton.addEventListener("click", this.handleCopy);

    toolbar.append(this.languageDropdownHost, this.wrapButton, this.copyButton);

    this.pre = document.createElement("pre");
    this.pre.className = "tiptap-code-scroll kosmos-scroll";
    this.pre.addEventListener("scroll", this.handleCodeScroll, { passive: true });
    this.pre.addEventListener("mouseenter", this.handleCodeHover);
    this.pre.addEventListener("mouseleave", this.handleCodeLeave);
    if (typeof ResizeObserver !== "undefined") {
      this.resizeObserver = new ResizeObserver(this.syncWrapAvailability);
      this.resizeObserver.observe(this.pre);
    }
    this.contentDOM = document.createElement("code");
    this.pre.append(this.contentDOM);

    this.expandButton = document.createElement("button");
    this.expandButton.type = "button";
    this.expandButton.className = "tiptap-code-expand";
    this.expandButton.contentEditable = "false";
    const expandIconHost = document.createElement("span");
    expandIconHost.className = "tiptap-code-expand-icon";
    this.expandIconApp = createApp({
      setup: () => () =>
        h(PhCaretDown, {
          size: 13,
          weight: "bold",
          "aria-hidden": "true",
        }),
    });
    this.expandIconApp.mount(expandIconHost);
    this.expandButton.append(expandIconHost);
    this.expandButton.addEventListener("click", this.handleExpandToggle);

    this.dom.append(toolbar, this.pre, this.expandButton);
    this.dom.addEventListener("click", this.handleCollapsedPreviewClick);
    this.syncControls(false);
  }

  update(node: ProseMirrorNode): boolean {
    if (node.type !== this.node.type) return false;
    this.node = node;
    this.syncControls(true);
    return true;
  }

  stopEvent(event: Event): boolean {
    return (
      event.target instanceof globalThis.Node &&
      (this.dom.querySelector(".tiptap-code-toolbar")?.contains(event.target) === true ||
        this.expandButton.contains(event.target))
    );
  }

  ignoreMutation(mutation: MutationRecord): boolean {
    return !this.contentDOM.contains(mutation.target);
  }

  destroy(): void {
    this.languageDropdownApp.unmount();
    this.wrapIconApp.unmount();
    this.copyIconApp.unmount();
    this.expandIconApp.unmount();
    this.wrapButton.removeEventListener("click", this.handleWrapToggle);
    this.copyButton.removeEventListener("click", this.handleCopy);
    this.expandButton.removeEventListener("click", this.handleExpandToggle);
    this.dom.removeEventListener("click", this.handleCollapsedPreviewClick);
    this.pre.removeEventListener("scroll", this.handleCodeScroll);
    this.pre.removeEventListener("mouseenter", this.handleCodeHover);
    this.pre.removeEventListener("mouseleave", this.handleCodeLeave);
    this.resizeObserver?.disconnect();
    if (this.scrollTimer !== null) clearTimeout(this.scrollTimer);
    if (this.copyTimer !== null) clearTimeout(this.copyTimer);
  }

  private readonly setLanguage = (language: string): void => {
    if (typeof this.getPos !== "function") return;
    const pos = this.getPos();
    if (typeof pos !== "number") return;
    const attrs = { ...this.node.attrs };
    if (language) attrs.language = language;
    else delete attrs.language;
    this.view.dispatch(this.view.state.tr.setNodeMarkup(pos, undefined, attrs));
    this.view.focus();
  };

  private readonly handleCopy = (): void => {
    void copyTextToClipboard(codeBlockMarkdownForClipboard(this.node)).then(() => {
      this.copyIconCopied.value = true;
      this.copyButton.classList.add("is-copied");
      if (this.copyTimer !== null) clearTimeout(this.copyTimer);
      this.copyTimer = setTimeout(() => {
        this.copyIconCopied.value = false;
        this.copyButton.classList.remove("is-copied");
        this.copyTimer = null;
      }, 900);
    });
  };

  private readonly handleWrapToggle = (): void => {
    this.wrapLines.value = !this.wrapLines.value;
    this.dom.classList.toggle("is-wrapped", this.wrapLines.value);
    this.wrapButton.title = this.wrapLines.value ? "Не переносить строки" : "Переносить строки";
    this.wrapButton.ariaLabel = this.wrapButton.title;
    if (this.wrapLines.value) this.pre.scrollLeft = 0;
    this.scheduleWrapAvailabilitySync();
    this.view.focus();
  };

  private readonly handleExpandToggle = (event?: MouseEvent): void => {
    event?.preventDefault();
    event?.stopPropagation();
    this.expanded = !this.expanded;
    this.syncExpandState();
    this.view.focus();
  };

  private readonly handleCollapsedPreviewClick = (event: MouseEvent): void => {
    if (!this.dom.classList.contains("is-collapsible") || this.expanded) return;
    if (!(event.target instanceof globalThis.Node)) return;
    if (
      this.languageDropdownHost.contains(event.target) ||
      this.copyButton.contains(event.target) ||
      this.expandButton.contains(event.target)
    ) {
      return;
    }

    event.preventDefault();
    event.stopPropagation();
    this.expanded = true;
    this.syncExpandState();
    this.view.focus();
  };

  private readonly handleCodeHover = (): void => {
    this.pre.dataset.scrolling = "1";
  };

  private readonly handleCodeLeave = (): void => {
    if (this.scrollTimer === null) delete this.pre.dataset.scrolling;
  };

  private readonly handleCodeScroll = (): void => {
    this.pre.dataset.scrolling = "1";
    if (this.scrollTimer !== null) clearTimeout(this.scrollTimer);
    this.scrollTimer = setTimeout(() => {
      this.scrollTimer = null;
      if (!this.pre.matches(":hover")) delete this.pre.dataset.scrolling;
    }, 900);
  };

  private syncControls(expandNewCollapsible: boolean): void {
    const language = visibleCodeLanguage(this.node);
    this.dom.dataset.language = language;
    this.languageDropdownValue.value = language;
    this.copyButton.disabled = this.node.textContent.length === 0;
    this.wrapButton.title = this.wrapLines.value ? "Не переносить строки" : "Переносить строки";
    this.wrapButton.ariaLabel = this.wrapButton.title;
    this.scheduleWrapAvailabilitySync();
    const isCollapsible =
      this.node.textContent.split(/\r?\n/).length > EdenCodeBlockView.COLLAPSED_LINE_LIMIT;
    const wasCollapsible = this.dom.classList.contains("is-collapsible");
    if (expandNewCollapsible && isCollapsible && !wasCollapsible) this.expanded = true;
    if (!isCollapsible) this.expanded = false;
    this.dom.classList.toggle("is-collapsible", isCollapsible);
    this.syncExpandState();
  }

  private syncExpandState(): void {
    const isCollapsible = this.dom.classList.contains("is-collapsible");
    this.dom.classList.toggle("is-expanded", isCollapsible && this.expanded);
    this.expandButton.hidden = !isCollapsible || this.expanded;
    if (isCollapsible && !this.expanded) this.pre.scrollTop = 0;
    this.expandButton.title = "Показать полностью";
    this.expandButton.ariaLabel = "Показать полностью";
    this.scheduleWrapAvailabilitySync();
  }

  private scheduleWrapAvailabilitySync(): void {
    window.requestAnimationFrame(this.syncWrapAvailability);
  }

  private readonly syncWrapAvailability = (): void => {
    if (this.dom.classList.contains("is-collapsible") && !this.expanded) {
      this.wrapButton.hidden = true;
      return;
    }
    this.wrapButton.hidden =
      !this.wrapLines.value && this.pre.scrollWidth <= this.pre.clientWidth + 1;
  };
}

const emit = defineEmits<Emits>();

const AUTOSAVE_DEBOUNCE_MS = 300;
const SLASH_MENU_GAP_PX = 16;
const SLASH_MENU_MAX_HEIGHT_PX = 300;
const DROPDOWN_MARGIN_PX = 4;
const SLASH_MENU_TRIGGER_SIZE_PX = 1;
const CODE_BLOCK_TAB_SIZE = 2;
const CODE_BLOCK_INDENT = " ".repeat(CODE_BLOCK_TAB_SIZE);
const CODE_BLOCK_PLUGIN_KEY = new PluginKey("eden-code-block-ui");
const JS_LIKE_RE = /\b(?:const|let|var|function|return|import|export|async|await)\b|=>/;

type CodeBlockLanguage = {
  value: string;
  label: string;
  aliases?: string[];
};

const CODE_BLOCK_LANGUAGES: CodeBlockLanguage[] = [
  { value: "", label: "Plain text" },
  { value: "bash", label: "Bash / Shell", aliases: ["sh", "shell", "zsh"] },
  { value: "c", label: "C" },
  { value: "cpp", label: "C++", aliases: ["c++", "cc", "cxx"] },
  { value: "csharp", label: "C Sharp", aliases: ["cs", "c#"] },
  { value: "css", label: "CSS" },
  { value: "diff", label: "Diff", aliases: ["patch"] },
  { value: "go", label: "Go", aliases: ["golang"] },
  { value: "graphql", label: "GraphQL", aliases: ["gql"] },
  { value: "html", label: "HTML" },
  { value: "ini", label: "INI", aliases: ["conf", "cfg"] },
  { value: "java", label: "Java" },
  {
    value: "javascript",
    label: "JavaScript",
    aliases: ["js", "jsx"],
  },
  { value: "json", label: "JSON" },
  { value: "kotlin", label: "Kotlin", aliases: ["kt", "kts"] },
  { value: "less", label: "Less" },
  { value: "lua", label: "Lua" },
  { value: "makefile", label: "Makefile", aliases: ["make"] },
  { value: "markdown", label: "Markdown", aliases: ["md"] },
  {
    value: "objective-c",
    label: "Objective-C",
    aliases: ["objc", "objectivec"],
  },
  { value: "perl", label: "Perl", aliases: ["pl"] },
  { value: "php", label: "PHP" },
  { value: "python", label: "Python", aliases: ["py"] },
  { value: "r", label: "R" },
  { value: "ruby", label: "Ruby", aliases: ["rb"] },
  { value: "rust", label: "Rust", aliases: ["rs"] },
  { value: "scss", label: "SCSS" },
  { value: "sql", label: "SQL" },
  { value: "swift", label: "Swift" },
  {
    value: "typescript",
    label: "TypeScript",
    aliases: ["ts", "tsx"],
  },
  { value: "xml", label: "XML" },
  { value: "yaml", label: "YAML", aliases: ["yml"] },
];

const languageAliasToValue = new Map<string, string>(
  CODE_BLOCK_LANGUAGES.flatMap((language) => [
    [language.value, language.value] as const,
    ...(language.aliases ?? []).map((alias) => [alias, language.value] as const),
  ]),
);

function codeBlockLanguageOptions(currentLanguage: string): CodeBlockLanguage[] {
  if (
    !currentLanguage ||
    CODE_BLOCK_LANGUAGES.some((language) => language.value === currentLanguage)
  ) {
    return CODE_BLOCK_LANGUAGES;
  }
  return [...CODE_BLOCK_LANGUAGES, { value: currentLanguage, label: currentLanguage }];
}

const hostRef = ref<HTMLDivElement | null>(null);
const titleShellRef = ref<HTMLDivElement | null>(null);
const titleInputRef = ref<HTMLInputElement | null>(null);
const title = ref(getEditableEntryTitle(props.entry.title, props.entry.header_props_json));
const currentTypeId = ref(props.entry.type_id ?? SYSTEM_TYPE_NOTE_ID);
const headerProps = ref<Record<string, unknown>>(
  safeParseHeaderProps(
    props.noteTypes.find((noteType) => noteType.id === currentTypeId.value) ?? null,
    props.entry.header_props_json,
  ),
);
const headerLayout = ref<string | null>(props.entry.header_layout);
const slashMenuOpen = ref(false);
const slashMenuX = ref(0);
const slashMenuY = ref(0);
const slashMenuValue = ref<string | null>(null);
const slashMenuTriggerRef = ref<HTMLDivElement | null>(null);
const suppressUpdate = ref(false);

let autosaveTimer: ReturnType<typeof setTimeout> | null = null;
let lastDraftUpdatedAt = props.entry.updated_at;
let lastPersistedTitle = props.entry.title;
let lastPersistedTypeId = props.entry.type_id ?? SYSTEM_TYPE_NOTE_ID;
let lastPersistedHeaderLayout = normalizedEntryHeaderLayout(props.entry);
let lastPersistedHeaderPropsJson = normalizeHeaderPropsJson(props.entry.header_props_json);
let lastPersistedBodyMarkdown = tiptapDocToMarkdown(readEntryTiptapDoc(props.entry.content_json));
let lastPersistedBodyJson = JSON.stringify(readEntryTiptapDoc(props.entry.content_json));
let bodyReadable = isReadableEntryContent(props.entry.content_json);
let bodyEditedSinceEntryLoad = false;
let titleOutOfView = false;

const activeNoteType = computed(
  () => props.noteTypes.find((noteType) => noteType.id === currentTypeId.value) ?? null,
);
const isPersonEntry = computed(() => currentTypeId.value === SYSTEM_TYPE_PERSON_ID);
const showTypedHeader = computed(() => Boolean(activeNoteType.value));
const showTitleInput = computed(() => !isPersonEntry.value);
const slashCommands = [
  { value: "h1", label: "Заголовок 1", icon: Heading1 },
  { value: "h2", label: "Заголовок 2", icon: Heading2 },
  { value: "h3", label: "Заголовок 3", icon: Heading3 },
  { value: "h4", label: "Заголовок 4", icon: Heading4 },
  { value: "h5", label: "Заголовок 5", icon: Heading5 },
  { value: "h6", label: "Заголовок 6", icon: Heading6 },
  { value: "text", label: "Текст", icon: Pilcrow },
  { value: "divider", label: "", kind: "separator" as const },
  { value: "bullet", label: "Маркированный список", icon: List },
  { value: "ordered", label: "Нумерованный список", icon: ListOrdered },
  { value: "task", label: "Задача", icon: CheckSquare },
  { value: "quote", label: "Цитата", icon: Quote },
  { value: "code", label: "Код", icon: Code },
  { value: "hr", label: "Разделитель", icon: Minus },
];

async function buildCodeBlockDecorations(doc: ProseMirrorNode): Promise<DecorationSet> {
  const decorations: Decoration[] = [];
  const jobs: Promise<void>[] = [];

  doc.descendants((node, pos) => {
    if (node.type.name !== "codeBlock") return;
    const start = pos + 1;
    const code = node.textContent;
    const language = syntaxLanguageForNode(node);
    if (!language) return;

    jobs.push(
      highlightCodeWithShiki(code, language).then((tokens) => {
        for (const token of tokens) {
          decorations.push(
            Decoration.inline(start + token.from, start + token.to, {
              style: shikiTokenStyle(token),
            }),
          );
        }
      }),
    );
  });

  await Promise.all(jobs);
  return DecorationSet.create(doc, decorations);
}

function shikiTokenStyle(token: { color?: string; fontStyle?: number }): string {
  const rules = token.color ? [`color: ${token.color}`] : [];
  if (token.fontStyle) {
    if (token.fontStyle & 1) rules.push("font-style: italic");
    if (token.fontStyle & 2) rules.push("font-weight: 600");
    if (token.fontStyle & 4) rules.push("text-decoration: underline");
  }
  return rules.join("; ");
}

function syntaxLanguageForNode(node: ProseMirrorNode): ShikiCodeLanguage | null {
  const language = normalizedCodeLanguage(node);
  if (isShikiCodeLanguage(language)) return language;
  if (!language && JS_LIKE_RE.test(node.textContent)) return "javascript";
  return null;
}

function visibleCodeLanguage(node: ProseMirrorNode): string {
  return normalizedCodeLanguage(node) || syntaxLanguageForNode(node) || "";
}

function codeBlockMarkdownForClipboard(node: ProseMirrorNode): string {
  const language = visibleCodeLanguage(node);
  const fence = longestBacktickRun(node.textContent) >= 3 ? "````" : "```";
  const code = node.textContent.endsWith("\n") ? node.textContent : `${node.textContent}\n`;
  return `${fence}${language}\n${code}${fence}`;
}

function longestBacktickRun(text: string): number {
  return Math.max(0, ...[...text.matchAll(/`+/g)].map((match) => match[0].length));
}

function normalizedCodeLanguage(node: ProseMirrorNode): string {
  const raw =
    typeof node.attrs.language === "string" ? node.attrs.language.trim().toLowerCase() : "";
  if (!raw) return "";
  return normalizeCodeLanguageValue(raw);
}

function normalizeCodeLanguageValue(raw: string | undefined): string {
  const normalized = raw?.trim().toLowerCase() ?? "";
  if (!normalized) return "";
  return languageAliasToValue.get(normalized) ?? normalized;
}

async function copyTextToClipboard(text: string): Promise<void> {
  try {
    if (navigator.clipboard?.writeText) {
      await navigator.clipboard.writeText(text);
      return;
    }
  } catch {
    // Fall through to the textarea path for WebView/permission edge cases.
  }

  const textarea = document.createElement("textarea");
  textarea.value = text;
  textarea.style.position = "fixed";
  textarea.style.left = "-9999px";
  document.body.append(textarea);
  textarea.select();
  document.execCommand("copy");
  textarea.remove();
}

const EdenRichTextEditing = Extension.create({
  name: "edenRichTextEditing",
  priority: 1000,

  addInputRules() {
    const rules = [];
    const { heading, codeBlock, blockquote, bulletList, orderedList } = this.editor.schema.nodes;

    if (heading) {
      rules.push(
        textblockTypeInputRule({
          find: /^(#{1,6})\s$/,
          type: heading,
          getAttributes: (match) => ({ level: match[1]?.length ?? 1 }),
        }),
      );
    }

    if (codeBlock) {
      rules.push(
        textblockTypeInputRule({
          find: /^```([a-zA-Z0-9_+#.-]*)\s$/,
          type: codeBlock,
          getAttributes: (match) => ({ language: normalizeCodeLanguageValue(match[1]) }),
        }),
      );
    }

    if (blockquote) {
      rules.push(wrappingInputRule({ find: /^\s*>\s$/, type: blockquote }));
    }

    if (bulletList) {
      rules.push(wrappingInputRule({ find: /^\s*([-+*])\s$/, type: bulletList }));
    }

    if (orderedList) {
      rules.push(
        wrappingInputRule({
          find: /^\s*(\d+)\.\s$/,
          type: orderedList,
          getAttributes: (match) => ({ start: Number(match[1] ?? 1) }),
        }),
      );
    }

    return rules;
  },

  addKeyboardShortcuts() {
    return {
      Backspace: () => {
        const { selection } = this.editor.state;
        if (!selection.empty || selection.$from.parentOffset !== 0) return false;
        if (selection.$from.parent.type.name !== "heading") return false;
        return this.editor.commands.setParagraph();
      },
    };
  },

  addProseMirrorPlugins() {
    const activeEditor = this.editor;
    return [
      new Plugin({
        key: new PluginKey("eden-code-copy-as-text"),
        props: {
          handleDOMEvents: {
            copy: (view, event) => handleEditorCopy(view, event),
            paste: (_view, event) => handleEditorPaste(activeEditor, event),
          },
        },
      }),
    ];
  },
});

function handleEditorPaste(activeEditor: Editor, event: Event): boolean {
  if (!(event instanceof ClipboardEvent) || !event.clipboardData) return false;
  const text = event.clipboardData.getData("text/plain");
  if (!hasCodeFence(text)) return false;

  const nodes = markdownToTiptapDoc(text).content ?? [];
  if (nodes.length === 0) return false;
  if (!activeEditor.commands.insertContent(nodes)) return false;

  event.preventDefault();
  return true;
}

function hasCodeFence(text: string): boolean {
  return /^(`{3,})[^\n`]*\n[\s\S]*\n\1\s*$/.test(text.trim());
}

function handleEditorCopy(view: EditorView, event: Event): boolean {
  if (!(event instanceof ClipboardEvent) || !event.clipboardData) return false;
  const range = selectedEditorTextRange(view);
  if (!range) return false;
  const codeBlock = selectedSingleCodeBlock(view, range);
  if (!codeBlock || codeBlock.isFullSelection) return false;

  event.clipboardData.setData("text/plain", view.state.doc.textBetween(range.from, range.to, "\n"));
  event.preventDefault();
  return true;
}

function selectedEditorTextRange(view: EditorView): { from: number; to: number } | null {
  const selection = view.state.selection;
  if (!selection.empty) return { from: selection.from, to: selection.to };

  const domSelection = view.dom.ownerDocument.getSelection();
  if (!domSelection || domSelection.isCollapsed || domSelection.rangeCount === 0) return null;
  const { anchorNode, focusNode } = domSelection;
  if (!anchorNode || !focusNode) return null;
  if (!view.dom.contains(anchorNode) || !view.dom.contains(focusNode)) return null;

  try {
    const anchor = view.posAtDOM(anchorNode, domSelection.anchorOffset);
    const focus = view.posAtDOM(focusNode, domSelection.focusOffset);
    if (anchor === focus) return null;
    return { from: Math.min(anchor, focus), to: Math.max(anchor, focus) };
  } catch {
    return null;
  }
}

function selectedSingleCodeBlock(
  view: EditorView,
  range: { from: number; to: number },
): { isFullSelection: boolean } | null {
  const $from = view.state.doc.resolve(range.from);
  const $to = view.state.doc.resolve(range.to);
  const maxDepth = Math.min($from.depth, $to.depth);

  for (let depth = maxDepth; depth > 0; depth--) {
    const node = $from.node(depth);
    if (node.type.name !== "codeBlock" || $to.node(depth) !== node) continue;
    const contentStart = $from.start(depth);
    const contentEnd = $from.end(depth);
    return {
      isFullSelection: range.from <= contentStart && range.to >= contentEnd,
    };
  }

  return null;
}

function activeCodeBlock(view: EditorView): { text: string; start: number } | null {
  const { selection } = view.state;
  const $from = selection.$from;
  const $to = selection.$to;
  for (let depth = Math.min($from.depth, $to.depth); depth > 0; depth--) {
    const node = $from.node(depth);
    if (node.type.name !== "codeBlock" || $to.node(depth) !== node) continue;
    return { text: node.textContent, start: $from.start(depth) };
  }
  return null;
}

function handleCodeBlockEnter(view: EditorView, event: KeyboardEvent): boolean {
  const block = activeCodeBlock(view);
  if (!block) return false;
  event.preventDefault();

  const { state } = view;
  const { from, to } = state.selection;
  const beforeCursor = block.text.slice(0, Math.max(0, from - block.start));
  const currentLine = beforeCursor.slice(beforeCursor.lastIndexOf("\n") + 1);
  const indent = /^\s*/.exec(currentLine)?.[0] ?? "";
  const extraIndent = /(?:[{[(]|:)\s*$/.test(currentLine) ? CODE_BLOCK_INDENT : "";
  view.dispatch(state.tr.insertText(`\n${indent}${extraIndent}`, from, to).scrollIntoView());
  return true;
}

async function openSlashMenu(): Promise<void> {
  const activeEditor = editor.value;
  if (!activeEditor) return;
  const rect = activeEditor.view.coordsAtPos(activeEditor.state.selection.from);
  const opensBelow =
    rect.bottom + SLASH_MENU_GAP_PX + SLASH_MENU_MAX_HEIGHT_PX <= window.innerHeight - 8;
  slashMenuX.value = rect.left;
  slashMenuY.value = opensBelow
    ? rect.bottom + SLASH_MENU_GAP_PX - DROPDOWN_MARGIN_PX - SLASH_MENU_TRIGGER_SIZE_PX
    : rect.top - SLASH_MENU_GAP_PX + DROPDOWN_MARGIN_PX;
  slashMenuOpen.value = true;
  slashMenuValue.value = null;
  await nextTick();
  slashMenuTriggerRef.value?.querySelector("button")?.click();
}

const editor = useEditor({
  content: readEntryTiptapDoc(props.entry.content_json),
  editable: !props.readerMode,
  extensions: [
    StarterKit.configure({
      codeBlock: {
        enableTabIndentation: true,
        tabSize: CODE_BLOCK_TAB_SIZE,
      },
    }),
    EdenRichTextEditing,
    EdenCodeBlockTools,
    EdenImage,
    TaskList,
    TaskItem.configure({ nested: true }),
    Placeholder.configure({ placeholder: "Начните писать..." }),
  ],
  editorProps: {
    attributes: {
      class: "ProseMirror tiptap-prosemirror",
      spellcheck: "false",
    },
    handleKeyDown(view, event) {
      const inCodeBlock = activeCodeBlock(view) !== null;
      if (inCodeBlock && event.key === "Enter") return handleCodeBlockEnter(view, event);
      if (event.key === "/" && !inCodeBlock && !props.readerMode) {
        void openSlashMenu();
      }
      if (event.key === "Escape") {
        slashMenuOpen.value = false;
      }
      return false;
    },
  },
  onUpdate({ editor: activeEditor }) {
    if (suppressUpdate.value || props.readerMode) return;
    bodyEditedSinceEntryLoad = true;
    const doc = activeEditor.getJSON() as TiptapDoc;
    emit("liveCharCount", activeEditor.getText().length);
    emit("entryDraftChange", buildEntryDraft(JSON.stringify(writeEntryTiptapDoc(doc))));
    scheduleAutosave();
  },
  onFocus() {
    slashMenuOpen.value = false;
  },
});

function getCurrentTitle(): string {
  return title.value;
}

function normalizeHeaderPropsJson(headerPropsJson: string | null | undefined): string {
  if (!headerPropsJson?.trim()) return JSON.stringify({});
  try {
    return JSON.stringify(JSON.parse(headerPropsJson) as Record<string, unknown>);
  } catch {
    return JSON.stringify({});
  }
}

function nextDraftUpdatedAt(): number {
  const now = Date.now();
  lastDraftUpdatedAt = Math.max(now, lastDraftUpdatedAt + 1);
  return lastDraftUpdatedAt;
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
    updated_at: nextDraftUpdatedAt(),
  };
}

function normalizedEntryHeaderLayout(entry: Entry): string | null {
  if (entry.header_layout !== null && entry.header_layout !== undefined) return entry.header_layout;
  const noteType =
    props.noteTypes.find((candidate) => candidate.id === (entry.type_id ?? SYSTEM_TYPE_NOTE_ID)) ??
    null;
  return resolveNoteTypeHeaderLayout(noteType);
}

function hasEntryDraftChanges(entry: Entry): boolean {
  if (entry.title !== lastPersistedTitle) return true;
  if ((entry.type_id ?? SYSTEM_TYPE_NOTE_ID) !== lastPersistedTypeId) return true;
  if (normalizedEntryHeaderLayout(entry) !== lastPersistedHeaderLayout) return true;
  if (normalizeHeaderPropsJson(entry.header_props_json) !== lastPersistedHeaderPropsJson) {
    return true;
  }
  return JSON.stringify(readEntryTiptapDoc(entry.content_json)) !== lastPersistedBodyJson;
}

function syncPersistedBaseline(entry: Entry): void {
  lastPersistedTitle = entry.title;
  lastPersistedTypeId = entry.type_id ?? SYSTEM_TYPE_NOTE_ID;
  lastPersistedHeaderLayout = normalizedEntryHeaderLayout(entry);
  lastPersistedHeaderPropsJson = normalizeHeaderPropsJson(entry.header_props_json);
  lastPersistedBodyMarkdown = tiptapDocToMarkdown(readEntryTiptapDoc(entry.content_json));
  lastPersistedBodyJson = JSON.stringify(readEntryTiptapDoc(entry.content_json));
  bodyReadable = isReadableEntryContent(entry.content_json);
  bodyEditedSinceEntryLoad = false;
}

function scheduleAutosave(): void {
  if (autosaveTimer !== null) clearTimeout(autosaveTimer);
  autosaveTimer = setTimeout(() => {
    autosaveTimer = null;
    void flushSave().catch((err) => {
      console.warn("[eden tiptap] autosave failed:", err);
    });
  }, AUTOSAVE_DEBOUNCE_MS);
}

async function flushSave(): Promise<void> {
  const activeEditor = editor.value;
  if (!activeEditor) return;
  const contentJson =
    bodyReadable || bodyEditedSinceEntryLoad
      ? JSON.stringify(writeEntryTiptapDoc(activeEditor.getJSON() as TiptapDoc))
      : props.entry.content_json;
  const entry = buildEntryDraft(contentJson);
  if (!hasEntryDraftChanges(entry)) return;
  emit("entryDraftChange", entry);
  const result = await props.onSave(entry);
  if (isFailedSaveResult(result)) {
    console.warn("[eden tiptap] save failed:", result);
    return;
  }
  syncPersistedBaseline(entry);
}

async function handleTypePick(nextTypeId: string): Promise<void> {
  const activeEditor = editor.value;
  const nextType = props.noteTypes.find((noteType) => noteType.id === nextTypeId) ?? null;
  if (!activeEditor || !nextType || nextTypeId === currentTypeId.value) return;

  currentTypeId.value = nextTypeId;
  headerProps.value = createHeaderPropsForTypeChange(nextType, getCurrentTitle());
  headerLayout.value = resolveNoteTypeHeaderLayout(nextType);
  const entry: Entry = {
    ...buildEntryDraft(JSON.stringify(writeEntryTiptapDoc(activeEditor.getJSON() as TiptapDoc))),
    type_id: nextTypeId,
    header_layout: headerLayout.value,
    header_props_json: JSON.stringify(headerProps.value),
  };

  emit("typeChange", entry);
  emit("entryDraftChange", entry);
  void window.api?.saveNoteType?.(nextType).catch((err) => {
    console.warn("[eden tiptap] type persist failed:", err);
  });
  const result = await props.onSave(entry);
  if (isFailedSaveResult(result)) {
    console.warn("[eden tiptap] type change save failed:", result);
    return;
  }
  syncPersistedBaseline(entry);
}

function handleHeaderPropChange(fieldId: string, value: unknown): void {
  headerProps.value = { ...headerProps.value, [fieldId]: value };
  emit("entryDraftChange", buildEntryDraft());
  scheduleAutosave();
}

function handleTitleInput(event: Event): void {
  title.value = (event.target as HTMLInputElement).value.replace(/[\r\n]+/g, " ");
  emit("entryDraftChange", buildEntryDraft());
  scheduleAutosave();
}

function syncTitleScrollState(): void {
  const titleHeight = titleShellRef.value?.offsetHeight ?? 0;
  const scrollTop = hostRef.value?.scrollTop ?? 0;
  const nextOutOfView = titleHeight > 0 && scrollTop >= titleHeight;
  if (nextOutOfView === titleOutOfView) return;
  titleOutOfView = nextOutOfView;
  emit("titleOutOfViewChange", nextOutOfView);
}

function focusBodyStartSoon(): void {
  void nextTick(() => {
    window.requestAnimationFrame(() => {
      editor.value?.commands.focus("start");
    });
  });
}

function syncBodyFromEntry(): void {
  const activeEditor = editor.value;
  if (!activeEditor) return;
  const nextDoc = readEntryTiptapDoc(props.entry.content_json);
  if (JSON.stringify(activeEditor.getJSON()) === JSON.stringify(nextDoc)) return;
  suppressUpdate.value = true;
  activeEditor.commands.setContent(nextDoc);
  suppressUpdate.value = false;
  emit("liveCharCount", activeEditor.getText().length);
}

function deleteSlashTrigger(): void {
  const activeEditor = editor.value;
  if (!activeEditor) return;
  const { from } = activeEditor.state.selection;
  activeEditor
    .chain()
    .focus()
    .deleteRange({ from: Math.max(0, from - 1), to: from })
    .run();
}

function runSlashCommand(kind: string): void {
  const activeEditor = editor.value;
  if (!activeEditor) return;
  deleteSlashTrigger();
  const chain = activeEditor.chain().focus();
  if (kind === "h1") chain.toggleHeading({ level: 1 }).run();
  else if (kind === "h2") chain.toggleHeading({ level: 2 }).run();
  else if (kind === "h3") chain.toggleHeading({ level: 3 }).run();
  else if (kind === "h4") chain.toggleHeading({ level: 4 }).run();
  else if (kind === "h5") chain.toggleHeading({ level: 5 }).run();
  else if (kind === "h6") chain.toggleHeading({ level: 6 }).run();
  else if (kind === "text") chain.setParagraph().run();
  else if (kind === "bullet") chain.toggleBulletList().run();
  else if (kind === "ordered") chain.toggleOrderedList().run();
  else if (kind === "task") chain.toggleTaskList().run();
  else if (kind === "quote") chain.toggleBlockquote().run();
  else if (kind === "code") chain.toggleCodeBlock().run();
  else if (kind === "hr") chain.setHorizontalRule().run();
  slashMenuOpen.value = false;
}

function handleSlashCommandPick(kind: string): void {
  slashMenuValue.value = kind;
  runSlashCommand(kind);
}

watch(
  () => props.entry.id,
  () => {
    const normalizedTypeId = props.entry.type_id ?? SYSTEM_TYPE_NOTE_ID;
    const noteType = props.noteTypes.find((candidate) => candidate.id === normalizedTypeId) ?? null;
    currentTypeId.value = normalizedTypeId;
    title.value = getEditableEntryTitle(props.entry.title, props.entry.header_props_json);
    headerProps.value = safeParseHeaderProps(noteType, props.entry.header_props_json);
    headerLayout.value = props.entry.header_layout ?? resolveNoteTypeHeaderLayout(noteType);
    titleOutOfView = false;
    emit("titleOutOfViewChange", false);
    syncBodyFromEntry();
    syncPersistedBaseline(props.entry);
    bodyReadable = isReadableEntryContent(props.entry.content_json);
    bodyEditedSinceEntryLoad = false;
    lastDraftUpdatedAt = props.entry.updated_at;
    if (!props.bodyLoading) focusBodyStartSoon();
  },
);

watch([() => props.entry.content_json, () => props.bodyLoading], () => {
  if (!props.bodyLoading) syncBodyFromEntry();
});

watch(
  () => props.readerMode,
  (enabled) => {
    editor.value?.setEditable(!enabled);
    if (!enabled) focusBodyStartSoon();
  },
);

watch(
  () => props.bodyLoading,
  (loading) => {
    if (!loading) focusBodyStartSoon();
  },
);

function handleHostScroll(): void {
  syncTitleScrollState();
}

onBeforeUnmount(() => {
  if (autosaveTimer !== null) {
    clearTimeout(autosaveTimer);
    autosaveTimer = null;
  }
  hostRef.value?.removeEventListener("scroll", handleHostScroll);
  void flushSave().catch((err) => {
    console.warn("[eden tiptap] unmount save failed:", err);
  });
  editor.value?.destroy();
});

onMounted(() => {
  hostRef.value?.addEventListener("scroll", handleHostScroll, {
    passive: true,
  });
  if (!props.bodyLoading) focusBodyStartSoon();
});
</script>

<template>
  <div
    ref="hostRef"
    class="tiptap-editor-host kosmos-scroll"
    :class="{
      'is-focus-mode': props.zenMode,
      'is-reader-mode': props.readerMode,
    }"
    data-testid="tiptap-editor-host"
  >
    <div ref="titleShellRef" class="tiptap-title-shell">
      <input
        v-if="showTitleInput"
        ref="titleInputRef"
        class="tiptap-title-input"
        :value="title"
        :readonly="props.readerMode"
        placeholder="Без названия"
        aria-label="Название заметки"
        @input="handleTitleInput"
        @blur="flushSave"
        @keydown.enter.prevent="editor?.commands.focus('start')"
      />
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
        :readonly="props.readerMode"
        :show-type-row="false"
        :show-title="isPersonEntry"
        @header-prop-change="handleHeaderPropChange"
        @object-type-change="handleTypePick"
        @relation-navigate="props.onNavigate"
      />
    </div>

    <div class="tiptap-body-shell">
      <EditorContent
        v-if="editor"
        v-show="!props.bodyLoading"
        :editor="editor"
        class="tiptap-editor-content"
      />
      <div
        v-if="props.bodyLoading"
        class="tiptap-body-skeleton"
        data-testid="tiptap-editor-body-skeleton"
        aria-label="Текст заметки загружается"
        aria-busy="true"
      >
        <Skeleton class="h-4 w-[92%]" />
        <Skeleton class="h-4 w-[78%]" />
        <Skeleton class="h-4 w-[86%]" />
        <Skeleton class="h-4 w-[54%]" />
        <Skeleton class="mt-5 h-28 w-full rounded-lg" />
      </div>
    </div>

    <div
      v-if="slashMenuOpen"
      ref="slashMenuTriggerRef"
      class="tiptap-slash-menu-anchor"
      :style="{ left: slashMenuX + 'px', top: slashMenuY + 'px' }"
    >
      <Dropdown
        :model-value="slashMenuValue"
        :options="slashCommands"
        placeholder="/"
        panel-align="start"
        :match-trigger-width="false"
        :show-chevron="false"
        :searchable="false"
        :max-height-px="300"
        @update:model-value="handleSlashCommandPick"
      >
        <template #option-leading="{ option }">
          <component
            :is="option.icon"
            class="tiptap-slash-menu-icon"
            :size="14"
            :stroke-width="2"
            aria-hidden="true"
          />
        </template>
      </Dropdown>
    </div>
  </div>
</template>

<style scoped>
.tiptap-editor-host {
  --tiptap-page-gutter: clamp(24px, 4vw, 40px);

  height: 100%;
  overflow: auto;
  overflow-x: hidden;
  overscroll-behavior: contain;
  color: var(--text-primary);
}

.tiptap-title-shell {
  box-sizing: border-box;
  width: 100%;
  max-width: 760px;
  margin: 0 auto;
  padding: 18px var(--tiptap-page-gutter) 8px;
  display: grid;
  gap: 10px;
}

.tiptap-title-input {
  width: 100%;
  border: 0;
  padding: 0;
  outline: none;
  background: transparent;
  color: var(--text-primary);
  font-family: var(--font-sans);
  font-size: 1.5em;
  font-weight: 700;
  line-height: 1.25;
}

.tiptap-title-input::placeholder {
  color: var(--text-tertiary);
}

.tiptap-title-shell :deep(.typed-object-header) {
  margin: 2px 0 8px;
  overflow: visible;
}

.tiptap-title-shell :deep(.typed-object-header__inner) {
  gap: 10px;
  padding: 0;
}

.tiptap-title-shell :deep(.typed-object-header.is-person .typed-object-header__hero) {
  gap: 12px;
}

.tiptap-title-shell :deep(.typed-object-header.is-person .typed-object-header__content) {
  gap: 8px;
}

.tiptap-title-shell :deep(.typed-object-header.is-person .typed-object-header__title) {
  max-width: 100%;
  color: var(--text-primary);
  font-family: "SF Pro Text", "SF Pro Display", Inter, var(--font-sans), system-ui, sans-serif;
  font-size: 28px;
  line-height: 32px;
  font-weight: 700;
  letter-spacing: -0.56px;
  text-wrap: balance;
}

.tiptap-title-shell :deep(.typed-object-header.is-person .typed-object-header__featured--column),
.tiptap-title-shell :deep(.typed-object-header.is-person .typed-object-header__secondary-list) {
  width: min(100%, 560px);
}

.tiptap-title-shell :deep(.typed-object-header__featured--column) {
  gap: 6px;
  padding-top: 4px;
}

.tiptap-title-shell :deep(.typed-object-header__secondary) {
  padding-top: 0;
}

.tiptap-title-shell :deep(.object-property-field--featured-column),
.tiptap-title-shell :deep(.object-property-field--secondary) {
  grid-template-columns: minmax(116px, 30%) minmax(0, 1fr);
  gap: 12px;
  min-height: 34px;
  padding: 3px 0;
}

.tiptap-title-shell :deep(.object-property-field--secondary:hover) {
  background: transparent;
}

.tiptap-title-shell :deep(.object-property-field__label),
.tiptap-title-shell :deep(.object-property-field__value),
.tiptap-title-shell :deep(.object-property-field__input),
.tiptap-title-shell :deep(.object-property-picker__trigger),
.tiptap-title-shell :deep(.object-property-picker__summary),
.tiptap-title-shell :deep(.object-property-picker__placeholder) {
  font-family:
    "SF Pro Text", "SF Pro Display", Inter, var(--font-sans), system-ui, sans-serif !important;
  font-size: 14px !important;
  font-weight: 400 !important;
  line-height: 22px !important;
  letter-spacing: -0.12px !important;
}

.tiptap-title-shell :deep(.object-property-field__input) {
  height: 30px;
  min-height: 30px;
  border-radius: 8px;
  background: transparent;
  caret-color: var(--eden-accent-color, var(--accent, currentColor));
}

.tiptap-title-shell :deep(.object-property-field__input::placeholder) {
  font: inherit;
  color: var(--muted-foreground);
  opacity: 1;
}

.tiptap-title-shell :deep(.object-property-picker__trigger) {
  height: 30px;
  min-height: 30px;
  padding: 0;
  background: transparent;
}

.tiptap-title-shell :deep(.typed-object-header__avatar-picker .object-property-picker__trigger) {
  width: 128px;
  height: 128px;
  min-height: 128px;
  place-items: center;
}

.tiptap-title-shell
  :deep(.typed-object-header__avatar-picker .object-property-picker__placeholder) {
  display: block;
  width: 100%;
  text-align: center;
  font-size: 30px;
  line-height: 1;
}

.tiptap-body-shell {
  position: relative;
  box-sizing: border-box;
  width: 100%;
  max-width: 760px;
  margin: 0 auto;
  padding: 16px var(--tiptap-page-gutter) 35vh;
}

.tiptap-editor-content {
  min-height: 0;
}

.tiptap-editor-content :deep(.ProseMirror) {
  min-height: 0;
  outline: none;
  caret-color: var(--eden-accent-color, var(--accent, currentColor));
  font-family: var(--font-sans);
  font-size: 16px;
  line-height: 1.7;
  letter-spacing: 0;
}

.tiptap-editor-content :deep(.ProseMirror::selection),
.tiptap-editor-content :deep(.ProseMirror *::selection) {
  background: color-mix(
    in srgb,
    var(--eden-accent-color, var(--accent, currentColor)) 22%,
    transparent
  );
}

.tiptap-editor-content :deep(.ProseMirror p) {
  margin: 0;
}

.tiptap-editor-content :deep(.ProseMirror > *) {
  margin-top: 0;
  margin-bottom: 27.2px;
}

.tiptap-editor-content :deep(.ProseMirror > :last-child) {
  margin-bottom: 0;
}

.tiptap-editor-content :deep(.ProseMirror h1),
.tiptap-editor-content :deep(.ProseMirror h2),
.tiptap-editor-content :deep(.ProseMirror h3),
.tiptap-editor-content :deep(.ProseMirror h4),
.tiptap-editor-content :deep(.ProseMirror h5),
.tiptap-editor-content :deep(.ProseMirror h6) {
  font-weight: 700;
  line-height: 1.4;
}

.tiptap-editor-content :deep(.ProseMirror h1) {
  font-size: 2em;
}

.tiptap-editor-content :deep(.ProseMirror h2) {
  font-size: 1.6em;
}

.tiptap-editor-content :deep(.ProseMirror h3) {
  font-size: 1.3em;
}

.tiptap-editor-content :deep(.ProseMirror h4) {
  font-size: 1.15em;
}

.tiptap-editor-content :deep(.ProseMirror h5) {
  font-size: 1em;
  letter-spacing: 0;
  text-transform: uppercase;
}

.tiptap-editor-content :deep(.ProseMirror h6) {
  color: var(--text-secondary);
  font-size: 0.92em;
}

.tiptap-editor-content :deep(.ProseMirror ul),
.tiptap-editor-content :deep(.ProseMirror ol) {
  padding-left: 0;
  list-style-position: inside;
}

.tiptap-editor-content :deep(.ProseMirror li p) {
  display: inline;
}

.tiptap-editor-content :deep(.ProseMirror li::marker) {
  color: currentColor;
}

.tiptap-editor-content :deep(.ProseMirror blockquote) {
  padding-left: 0;
  border-left: 0;
  color: inherit;
  font-style: italic;
}

.tiptap-editor-content :deep(.ProseMirror code) {
  border-radius: 4px;
  background: transparent;
  font-family: var(--font-mono);
  font-size: 0.92em;
  padding: 1px 4px;
}

.tiptap-editor-content :deep(.ProseMirror pre) {
  overflow: auto;
  padding: 12px 14px;
  border-radius: 8px;
  background: var(--color-shape-highlight-light-solid);
  font-family: var(--font-mono);
  font-size: 13px;
}

.tiptap-editor-content :deep(.ProseMirror .tiptap-code-block) {
  position: relative;
  overflow: hidden;
  width: 100%;
  border: 0;
  border-radius: 8px;
  background: color-mix(in srgb, var(--surface-secondary, #242424) 96%, #000 4%);
}

.tiptap-editor-content :deep(.ProseMirror .tiptap-code-toolbar) {
  position: absolute;
  top: 6px;
  right: 8px;
  z-index: 2;
  display: inline-flex;
  align-items: center;
  gap: 6px;
  user-select: none;
}

.tiptap-editor-content :deep(.ProseMirror .tiptap-code-language-host) {
  display: flex;
  justify-content: flex-end;
  width: 138px;
}

.tiptap-editor-content :deep(.ProseMirror .tiptap-code-language-dropdown button) {
  width: 100%;
  height: 26px;
  border: 1px solid transparent;
  padding: 0 8px;
  color: var(--text-primary);
  font-size: 12px;
  transition: border-color 140ms ease;
}

.tiptap-editor-content :deep(.ProseMirror .tiptap-code-language-dropdown button:hover) {
  border-color: color-mix(in srgb, var(--text-secondary) 48%, transparent);
}

.tiptap-editor-content :deep(.ProseMirror .tiptap-code-language-dropdown button:focus-visible),
.tiptap-editor-content :deep(.ProseMirror .tiptap-code-action:focus-visible),
.tiptap-editor-content :deep(.ProseMirror .tiptap-code-expand:focus-visible) {
  outline: 2px solid
    color-mix(in srgb, var(--eden-accent-color, var(--accent, currentColor)) 65%, transparent);
  outline-offset: 2px;
}

.tiptap-editor-content :deep(.ProseMirror .tiptap-code-action) {
  position: relative;
  display: inline-flex;
  align-items: center;
  justify-content: center;
  width: 26px;
  height: 26px;
  border: 0;
  border-radius: 5px;
  padding: 0;
  background: transparent;
  color: var(--text-secondary);
  cursor: default;
  transition:
    background-color 140ms ease,
    color 140ms ease;
}

.tiptap-editor-content :deep(.ProseMirror .tiptap-code-action:hover) {
  background: color-mix(in srgb, var(--text-primary) 8%, transparent);
  color: var(--text-primary);
}

.tiptap-editor-content :deep(.ProseMirror .tiptap-code-copy:disabled) {
  cursor: default;
  opacity: 0.35;
}

.tiptap-editor-content :deep(.ProseMirror .tiptap-code-copy.is-copied) {
  color: var(--eden-accent-color, var(--accent, currentColor));
}

.tiptap-editor-content :deep(.ProseMirror .tiptap-code-wrap[hidden]) {
  display: none;
}

.tiptap-editor-content :deep(.ProseMirror .tiptap-code-block pre) {
  margin: 0;
  padding: 42px 14px 18px;
  border-radius: 0;
  background: transparent;
  cursor: default;
}

.tiptap-editor-content :deep(.ProseMirror .tiptap-code-block code) {
  display: block;
  min-width: max-content;
  border-radius: 0;
  background: transparent;
  color: var(--text-primary);
  font-family: var(--font-mono);
  font-size: 13px;
  line-height: 1.65;
  padding: 0;
  tab-size: 2;
  white-space: pre;
}

.tiptap-editor-content :deep(.ProseMirror .tiptap-code-block.is-wrapped pre) {
  overflow-x: hidden;
}

.tiptap-editor-content :deep(.ProseMirror .tiptap-code-block.is-wrapped code) {
  min-width: 0;
  width: 100%;
  overflow-wrap: break-word;
  white-space: pre-wrap;
  word-break: normal;
}

.tiptap-editor-content
  :deep(.ProseMirror .tiptap-code-block.is-collapsible:not(.is-expanded)::after) {
  position: absolute;
  right: 0;
  bottom: 0;
  left: 0;
  height: 72px;
  background: linear-gradient(
    to bottom,
    transparent,
    color-mix(in srgb, var(--surface-secondary, #242424) 96%, #000 4%) 72%
  );
  content: "";
  pointer-events: none;
}

.tiptap-editor-content :deep(.ProseMirror .tiptap-code-block.is-collapsible:not(.is-expanded) pre) {
  max-height: 360px;
  overflow-y: hidden;
  padding-bottom: 44px;
}

.tiptap-editor-content :deep(.ProseMirror .tiptap-code-expand) {
  position: absolute;
  left: 50%;
  bottom: 10px;
  z-index: 3;
  display: inline-flex;
  align-items: center;
  justify-content: center;
  width: 30px;
  height: 26px;
  border: 1px solid var(--border-color-strong, var(--border));
  border-radius: 6px;
  padding: 0;
  background: var(--settings-search-surface, var(--popover, var(--background)));
  color: var(--text-secondary);
  cursor: default;
  font-size: 12px;
  line-height: 1;
  transform: translateX(-50%);
  transition:
    background-color 140ms ease,
    border-color 140ms ease,
    color 140ms ease;
}

.tiptap-editor-content :deep(.ProseMirror .tiptap-code-expand:hover) {
  border-color: color-mix(in srgb, var(--text-secondary) 48%, transparent);
  color: var(--text-primary);
}

.tiptap-editor-content :deep(.ProseMirror .tiptap-code-expand[hidden]) {
  display: none;
}

.tiptap-slash-menu-anchor {
  position: fixed;
  z-index: 9500;
  width: 1px;
  height: 1px;
  line-height: 0;
}

.tiptap-slash-menu-anchor :deep(> .relative) {
  position: absolute;
  left: 0;
  top: 0;
  width: 1px;
  height: 1px;
  line-height: 0;
}

.tiptap-slash-menu-anchor :deep(button) {
  width: 1px !important;
  min-width: 1px !important;
  height: 1px !important;
  padding: 0 !important;
  opacity: 0;
}

.tiptap-slash-menu-icon {
  color: var(--text-secondary);
}

.tiptap-body-skeleton {
  display: grid;
  gap: 12px;
}
</style>
