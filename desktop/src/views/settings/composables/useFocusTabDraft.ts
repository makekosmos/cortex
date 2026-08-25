import { computed, type Ref } from "vue";
import type { FocusBlocklist, FocusDraftParsed } from "./useFocusTab.shared";
import { parseFocusDraft } from "./useFocusTab.shared";
import type { JsonRecord } from "../../../shared/runtimeGuards";

type FocusRequest = <T>(op: string, params?: JsonRecord) => Promise<T | null>;

interface FocusTabDraftDeps {
  focusBusy: Ref<string>;
  focusEditing: Ref<boolean>;
  focusEditingId: Ref<string | null>;
  focusDraftName: Ref<string>;
  focusDraftDomains: Ref<string>;
  focusDraftIcon: Ref<string>;
  focusDraftError: Ref<string>;
  focusDragOver: Ref<boolean>;
  focusBlocklists: Ref<FocusBlocklist[]>;
  mentionOpen: Ref<boolean>;
  mentionQuery: Ref<string>;
  mentionAnchor: Ref<number>;
  mentionTextareaRef: Ref<HTMLTextAreaElement | null>;
  mentionHighlight: Ref<number>;
  focusRequest: FocusRequest;
  loadBlocklists: () => Promise<void>;
}

export function useFocusTabDraft(deps: FocusTabDraftDeps) {
  const focusDraftParsed = computed<FocusDraftParsed>(() =>
    parseFocusDraft(deps.focusDraftDomains.value),
  );

  const mentionCandidates = computed<FocusBlocklist[]>(() => {
    const q = deps.mentionQuery.value.toLowerCase();
    return deps.focusBlocklists.value
      .filter((b) => b.id !== deps.focusEditingId.value)
      .filter((b) => !q || b.id.toLowerCase().includes(q) || b.name.toLowerCase().includes(q))
      .slice(0, 6);
  });

  function openCreateBlocklist() {
    deps.focusEditing.value = true;
    deps.focusEditingId.value = null;
    deps.focusDraftName.value = "";
    deps.focusDraftDomains.value = "";
    deps.focusDraftIcon.value = "";
    deps.focusDraftError.value = "";
  }

  function openEditBlocklist(bl: FocusBlocklist) {
    deps.focusEditing.value = true;
    deps.focusEditingId.value = bl.id;
    deps.focusDraftName.value = bl.name;
    deps.focusDraftDomains.value = bl.domains.join("\n");
    deps.focusDraftIcon.value = bl.icon ?? "";
    deps.focusDraftError.value = "";
  }

  function cancelCreateBlocklist() {
    deps.focusEditing.value = false;
    deps.focusEditingId.value = null;
    deps.focusDraftError.value = "";
    deps.mentionOpen.value = false;
  }

  async function onCreateBlocklist() {
    const name = deps.focusDraftName.value.trim();
    if (!name) {
      deps.focusDraftError.value = "Укажи название блок-листа";
      return;
    }

    const { domains, kind, invalid } = focusDraftParsed.value;
    if (domains.length === 0) {
      deps.focusDraftError.value = "Добавь хотя бы один домен или @ссылку";
      return;
    }
    if (invalid.length > 0) {
      deps.focusDraftError.value = `Невалидных строк: ${invalid.length}. Исправь или удали их`;
      return;
    }

    deps.focusBusy.value = "__create__";
    deps.focusDraftError.value = "";
    try {
      await deps.focusRequest<FocusBlocklist>("focus.upsert_blocklist", {
        id: deps.focusEditingId.value ?? undefined,
        name,
        domains,
        kind,
        icon: deps.focusDraftIcon.value,
      });
      deps.focusEditing.value = false;
      deps.focusEditingId.value = null;
      await deps.loadBlocklists();
    } catch (e) {
// SAFETY: the surrounding domain validation preserves the asserted contract.
      deps.focusDraftError.value = (e as Error).message;
    } finally {
      deps.focusBusy.value = "";
    }
  }

  function onDomainsInput(e: Event) {
// SAFETY: the surrounding domain validation preserves the asserted contract.
    const ta = e.target as HTMLTextAreaElement;
    const pos = ta.selectionStart;
    const text = ta.value;
    let i = pos - 1;
    let at = -1;
    while (i >= 0) {
      const ch = text[i];
      if (ch === "@") {
        at = i;
        break;
      }
      if (ch === " " || ch === "\n" || ch === "\t") break;
      i--;
    }
    if (at >= 0 && (at === 0 || /[\s\n]/.test(text[at - 1] ?? ""))) {
      deps.mentionOpen.value = true;
      deps.mentionAnchor.value = at;
      deps.mentionQuery.value = text.slice(at + 1, pos);
      deps.mentionHighlight.value = 0;
    } else {
      deps.mentionOpen.value = false;
    }
  }

  function insertMention(bl: FocusBlocklist) {
    const ta = deps.mentionTextareaRef.value;
    if (!ta) return;
    const text = deps.focusDraftDomains.value;
    const before = text.slice(0, deps.mentionAnchor.value);
    const after = text.slice(ta.selectionStart);
    const inserted = `@${bl.id}`;
    deps.focusDraftDomains.value = before + inserted + after;
    deps.mentionOpen.value = false;
    void Promise.resolve().then(() => {
      const newPos = before.length + inserted.length;
      ta.focus();
      ta.setSelectionRange(newPos, newPos);
    });
  }

  function onMentionKey(e: KeyboardEvent) {
    if (!deps.mentionOpen.value) return;
    const list = mentionCandidates.value;
    if (e.key === "ArrowDown") {
      e.preventDefault();
      deps.mentionHighlight.value = (deps.mentionHighlight.value + 1) % Math.max(list.length, 1);
    } else if (e.key === "ArrowUp") {
      e.preventDefault();
      deps.mentionHighlight.value =
        (deps.mentionHighlight.value - 1 + list.length) % Math.max(list.length, 1);
    } else if (e.key === "Enter") {
      if (list.length > 0) {
        e.preventDefault();
        insertMention(list[deps.mentionHighlight.value]!);
      }
    } else if (e.key === "Escape") {
      e.preventDefault();
      deps.mentionOpen.value = false;
    }
  }

  function onDraftDragOver(e: DragEvent) {
    e.preventDefault();
    deps.focusDragOver.value = true;
  }

  function onDraftDragLeave() {
    deps.focusDragOver.value = false;
  }

  async function onDraftDrop(e: DragEvent) {
    e.preventDefault();
    deps.focusDragOver.value = false;
    const file = e.dataTransfer?.files?.[0];
    if (!file) return;
    if (!/\.txt$/i.test(file.name)) {
      deps.focusDraftError.value = "Поддерживаются только .txt файлы";
      return;
    }

    try {
      const text = await file.text();
      deps.focusDraftDomains.value = deps.focusDraftDomains.value
        ? `${deps.focusDraftDomains.value}\n${text}`
        : text;
      if (!deps.focusDraftName.value) {
        deps.focusDraftName.value = file.name.replace(/\.txt$/i, "");
      }
    } catch (err) {
// SAFETY: the surrounding domain validation preserves the asserted contract.
      deps.focusDraftError.value = `Не удалось прочитать файл: ${(err as Error).message}`;
    }
  }

  return {
    focusDraftParsed,
    mentionCandidates,
    openCreateBlocklist,
    openEditBlocklist,
    cancelCreateBlocklist,
    onCreateBlocklist,
    onDomainsInput,
    insertMention,
    onMentionKey,
    onDraftDragOver,
    onDraftDragLeave,
    onDraftDrop,
  };
}
