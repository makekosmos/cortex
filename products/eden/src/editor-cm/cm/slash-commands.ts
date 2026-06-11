// Портировано из ZenNotes (MIT, © 2026 Adib Hanna and ZenNotes contributors), адаптировано для Eden.
import type { CompletionContext, CompletionResult, Completion } from "@codemirror/autocomplete";
import type { EditorView } from "@codemirror/view";

/**
 * Упрощено против оригинала:
 * - Удалена команда "Page" (требует store/навигации)
 * - Удалена templateSlashCommandSource (не нужна в Eden)
 * - Удалены зависимости useStore/zustand
 * - Лейблы команд — на русском языке
 */

interface SlashCmd {
  label: string;
  detail: string;
  icon: string;
  insert: string;
  /** Cursor offset from end of inserted text. Negative = move back. */
  cursorOffset?: number;
}

type DecoratedCompletion = Completion & {
  _kind?: "slash";
  _icon?: string;
};

const COMMANDS: SlashCmd[] = [
  { label: "Заголовок 1", detail: "#", icon: "H1", insert: "# " },
  { label: "Заголовок 2", detail: "##", icon: "H2", insert: "## " },
  { label: "Заголовок 3", detail: "###", icon: "H3", insert: "### " },
  { label: "Маркированный список", detail: "-", icon: "•", insert: "- " },
  { label: "Нумерованный список", detail: "1.", icon: "1.", insert: "1. " },
  { label: "Список задач", detail: "[ ]", icon: "☐", insert: "- [ ] " },
  { label: "Цитата", detail: ">", icon: "❝", insert: "> " },
  { label: "Блок кода", detail: "```", icon: "</>", insert: "```\n\n```", cursorOffset: -4 },
  { label: "Разделитель", detail: "---", icon: "—", insert: "---\n" },
  {
    label: "Таблица",
    detail: "|",
    icon: "⊞",
    insert: "| Колонка 1 | Колонка 2 |\n| --- | --- |\n| | |",
  },
  { label: "Формула", detail: "$$", icon: "∑", insert: "$$\n\n$$", cursorOffset: -3 },
  { label: "Ссылка", detail: "[]", icon: "🔗", insert: "[]()", cursorOffset: -3 },
];

/** Render a custom completion item matching the Eden theme. */
function renderCompletion(completion: Completion): HTMLElement {
  const decorated = completion as DecoratedCompletion;

  const el = document.createElement("div");
  el.className = "slash-cmd-item";

  const icon = document.createElement("span");
  icon.className = "slash-cmd-icon";
  icon.textContent = decorated._icon ?? "";

  const label = document.createElement("span");
  label.className = "slash-cmd-label";
  label.textContent = completion.label;

  const detail = document.createElement("span");
  detail.className = "slash-cmd-detail";
  detail.textContent = completion.detail ?? "";

  el.appendChild(icon);
  el.appendChild(label);
  el.appendChild(detail);
  return el;
}

/**
 * CodeMirror completion source for Notion-style slash commands.
 * Activates when `/` is typed at the start of a line or after whitespace.
 */
export function slashCommandSource(context: CompletionContext): CompletionResult | null {
  const { state, pos } = context;
  const line = state.doc.lineAt(pos);
  const textBefore = state.doc.sliceString(line.from, pos);

  // Match / at start of line or after whitespace, plus optional filter text
  const match = textBefore.match(/(?:^|\s)(\/[^\s]*)$/);
  if (!match) return null;

  const slashStart = pos - match[1].length; // position of /

  return {
    from: slashStart + 1, // position after / (for filtering)
    options: COMMANDS.map(
      (cmd): Completion =>
        ({
          label: cmd.label,
          detail: cmd.detail,
          _kind: "slash",
          _icon: cmd.icon,
          type: "slash",
          apply: (view: EditorView, _completion: Completion, _from: number, to: number) => {
            const insert = cmd.insert;
            const cursorPos =
              cmd.cursorOffset != null
                ? slashStart + insert.length + cmd.cursorOffset
                : slashStart + insert.length;
            view.dispatch({
              changes: { from: slashStart, to, insert },
              selection: { anchor: cursorPos },
            });
          },
        }) as Completion & { _icon: string },
    ),
    filter: true,
  };
}

/** Custom rendering for the slash command completion items. */
export const slashCommandRender = {
  render: renderCompletion,
};
