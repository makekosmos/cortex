// useFocusTab — state + handlers Focus tab'а: список блок-листов + activate /
// deactivate + системный демон + drafting (create / edit) + @-mention автокомплит
// в textarea доменов + drag-drop .txt файлов.

import { computed, ref, type InjectionKey } from "vue";

export interface FocusBlocklist {
  id: string;
  name: string;
  domains: string[];
  createdAt: string;
  preset?: boolean;
  icon?: string;
  kind?: "domains" | "raw";
}

export interface FocusActiveState {
  active: boolean;
  blocklist_id?: string | null;
  started_at?: string | null;
}

export const ICON_CHOICES = [
  "🛡️",
  "🚫",
  "🎮",
  "🧠",
  "📰",
  "📺",
  "🎬",
  "💬",
  "🐦",
  "📷",
  "🛒",
  "⚽",
  "🎰",
  "🍔",
  "💸",
  "🎵",
  "📚",
  "⚙️",
  "🔒",
  "🎯",
  "⏰",
  "🌐",
  "✨",
  "🔥",
  "⚡",
] as const;

const DOMAIN_PATTERN = /^[a-z0-9][a-z0-9.-]*\.[a-z]{2,}$/i;

function isUnknownOperationError(err: unknown): boolean {
  const msg = (err as Error)?.message ?? String(err);
  return /unknown operation|unknown_operation|not.?found/i.test(msg);
}

/**
 * Парсит текст blocklist textarea в `{ domains, kind, invalid }`.
 *
 * - Строки начинающиеся с `#` — комментарии, игнорируются.
 * - Строки начинающиеся с `@` — references на другой blocklist (валидируем
 *   что это `@<id>`). Их наличие → kind="raw".
 * - Остальное — должно быть валидным доменом.
 */
function parseRawList(raw: string): {
  domains: string[];
  kind: "domains" | "raw";
  invalid: string[];
} {
  const lines = raw.split("\n");
  const valid: string[] = [];
  const invalid: string[] = [];
  let hasReferences = false;
  for (const line of lines) {
    const stripped = line.replace(/#.*$/, "").trim();
    if (!stripped) continue;
    if (stripped.startsWith("@")) {
      const refId = stripped.slice(1).trim();
      if (!refId) {
        invalid.push(stripped);
        continue;
      }
      hasReferences = true;
      valid.push(`@${refId}`);
    } else if (DOMAIN_PATTERN.test(stripped)) {
      valid.push(stripped.toLowerCase());
    } else {
      invalid.push(stripped);
    }
  }
  return {
    domains: valid,
    kind: hasReferences ? "raw" : "domains",
    invalid,
  };
}

export function useFocusTab() {
  const focusBlocklists = ref<FocusBlocklist[]>([]);
  const focusLoading = ref<boolean>(false);
  const focusError = ref<string>("");
  const focusBackendMissing = ref<boolean>(false);
  const focusActive = ref<FocusActiveState>({ active: false });
  const focusBusy = ref<string>("");

  const focusServiceStatus = ref<{ installed: boolean; running: boolean }>({
    installed: false,
    running: false,
  });
  const focusServiceBusy = ref<string>("");
  const focusServiceError = ref<string>("");

  const focusEditing = ref<boolean>(false);
  const focusEditingId = ref<string | null>(null);
  const focusDraftName = ref<string>("");
  const focusDraftDomains = ref<string>("");
  const focusDraftIcon = ref<string>("");
  const focusDraftError = ref<string>("");
  const focusDragOver = ref<boolean>(false);

  const mentionOpen = ref<boolean>(false);
  const mentionQuery = ref<string>("");
  const mentionAnchor = ref<number>(0);
  const mentionTextareaRef = ref<HTMLTextAreaElement | null>(null);
  const mentionHighlight = ref<number>(0);

  async function focusRequest<T>(op: string, params?: Record<string, unknown>): Promise<T | null> {
    try {
      return await window.kepler.ark.request<T>(op, params);
    } catch (e) {
      if (isUnknownOperationError(e)) {
        focusBackendMissing.value = true;
        return null;
      }
      throw e;
    }
  }

  async function refreshFocusServiceStatus() {
    try {
      focusServiceStatus.value = await window.kepler.focusService.status();
    } catch (e) {
      console.warn("focus-service status failed", e);
    }
  }

  async function installFocusService() {
    focusServiceBusy.value = "install";
    focusServiceError.value = "";
    try {
      const r = await window.kepler.focusService.install();
      if (!r.ok) focusServiceError.value = r.error ?? "Установка не удалась";
      await refreshFocusServiceStatus();
    } finally {
      focusServiceBusy.value = "";
    }
  }

  async function uninstallFocusService() {
    focusServiceBusy.value = "uninstall";
    focusServiceError.value = "";
    try {
      const r = await window.kepler.focusService.uninstall();
      if (!r.ok) focusServiceError.value = r.error ?? "Удаление не удалось";
      await refreshFocusServiceStatus();
    } finally {
      focusServiceBusy.value = "";
    }
  }

  async function loadBlocklists() {
    focusLoading.value = true;
    focusError.value = "";
    try {
      const r = await focusRequest<{ blocklists: FocusBlocklist[] }>("focus.list_blocklists");
      focusBlocklists.value = r?.blocklists ?? [];
    } catch (e) {
      focusError.value = (e as Error).message;
      focusBlocklists.value = [];
    } finally {
      focusLoading.value = false;
    }
  }

  async function loadActiveState() {
    try {
      const r = await focusRequest<FocusActiveState>("focus.get_active_state");
      focusActive.value = r ?? { active: false };
    } catch (e) {
      console.warn("focus.get_active_state failed", e);
    }
  }

  const focusDraftParsed = computed(() => parseRawList(focusDraftDomains.value));

  const mentionCandidates = computed<FocusBlocklist[]>(() => {
    const q = mentionQuery.value.toLowerCase();
    return focusBlocklists.value
      .filter((b) => b.id !== focusEditingId.value)
      .filter((b) => !q || b.id.toLowerCase().includes(q) || b.name.toLowerCase().includes(q))
      .slice(0, 6);
  });

  const focusActiveBlocklist = computed<FocusBlocklist | undefined>(() => {
    if (!focusActive.value.active || !focusActive.value.blocklist_id) return undefined;
    return focusBlocklists.value.find((b) => b.id === focusActive.value.blocklist_id);
  });

  function openCreateBlocklist() {
    focusEditing.value = true;
    focusEditingId.value = null;
    focusDraftName.value = "";
    focusDraftDomains.value = "";
    focusDraftIcon.value = "";
    focusDraftError.value = "";
  }

  function openEditBlocklist(bl: FocusBlocklist) {
    focusEditing.value = true;
    focusEditingId.value = bl.id;
    focusDraftName.value = bl.name;
    focusDraftDomains.value = bl.domains.join("\n");
    focusDraftIcon.value = bl.icon ?? "";
    focusDraftError.value = "";
  }

  function cancelCreateBlocklist() {
    focusEditing.value = false;
    focusEditingId.value = null;
    focusDraftError.value = "";
    mentionOpen.value = false;
  }

  async function onCreateBlocklist() {
    const name = focusDraftName.value.trim();
    if (!name) {
      focusDraftError.value = "Укажи название блок-листа";
      return;
    }
    const { domains, kind, invalid } = focusDraftParsed.value;
    if (domains.length === 0) {
      focusDraftError.value = "Добавь хотя бы один домен или @ссылку";
      return;
    }
    if (invalid.length > 0) {
      focusDraftError.value = `Невалидных строк: ${invalid.length}. Исправь или удали их`;
      return;
    }
    focusBusy.value = "__create__";
    focusDraftError.value = "";
    try {
      await focusRequest<FocusBlocklist>("focus.upsert_blocklist", {
        id: focusEditingId.value ?? undefined,
        name,
        domains,
        kind,
        icon: focusDraftIcon.value,
      });
      focusEditing.value = false;
      focusEditingId.value = null;
      await loadBlocklists();
    } catch (e) {
      focusDraftError.value = (e as Error).message;
    } finally {
      focusBusy.value = "";
    }
  }

  function onDomainsInput(e: Event) {
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
      mentionOpen.value = true;
      mentionAnchor.value = at;
      mentionQuery.value = text.slice(at + 1, pos);
      mentionHighlight.value = 0;
    } else {
      mentionOpen.value = false;
    }
  }

  function insertMention(bl: FocusBlocklist) {
    const ta = mentionTextareaRef.value;
    if (!ta) return;
    const text = focusDraftDomains.value;
    const before = text.slice(0, mentionAnchor.value);
    const after = text.slice(ta.selectionStart);
    const inserted = `@${bl.id}`;
    focusDraftDomains.value = before + inserted + after;
    mentionOpen.value = false;
    void Promise.resolve().then(() => {
      const newPos = before.length + inserted.length;
      ta.focus();
      ta.setSelectionRange(newPos, newPos);
    });
  }

  function onMentionKey(e: KeyboardEvent) {
    if (!mentionOpen.value) return;
    const list = mentionCandidates.value;
    if (e.key === "ArrowDown") {
      e.preventDefault();
      mentionHighlight.value = (mentionHighlight.value + 1) % Math.max(list.length, 1);
    } else if (e.key === "ArrowUp") {
      e.preventDefault();
      mentionHighlight.value =
        (mentionHighlight.value - 1 + list.length) % Math.max(list.length, 1);
    } else if (e.key === "Enter") {
      if (list.length > 0) {
        e.preventDefault();
        insertMention(list[mentionHighlight.value]!);
      }
    } else if (e.key === "Escape") {
      e.preventDefault();
      mentionOpen.value = false;
    }
  }

  async function onDeleteBlocklist(id: string) {
    if (focusBusy.value) return;
    focusBusy.value = id;
    focusError.value = "";
    try {
      await focusRequest<{ ok: boolean }>("focus.delete_blocklist", { id });
      if (focusActive.value.blocklist_id === id) {
        focusActive.value = { active: false };
      }
      await loadBlocklists();
    } catch (e) {
      focusError.value = (e as Error).message;
    } finally {
      focusBusy.value = "";
    }
  }

  async function onDeactivate() {
    focusBusy.value = "__deactivate__";
    focusError.value = "";
    try {
      await focusRequest<{ ok: boolean }>("focus.set_active_state", {
        active: false,
      });
      focusActive.value = { active: false };
    } catch (e) {
      focusError.value = (e as Error).message;
    } finally {
      focusBusy.value = "";
    }
  }

  async function onActivate(id: string) {
    focusBusy.value = id;
    focusError.value = "";
    try {
      await focusRequest<{ ok: boolean }>("focus.set_active_state", {
        active: true,
        blocklist_id: id,
      });
      await loadActiveState();
    } catch (e) {
      focusError.value = (e as Error).message;
    } finally {
      focusBusy.value = "";
    }
  }

  function onDraftDragOver(e: DragEvent) {
    e.preventDefault();
    focusDragOver.value = true;
  }

  function onDraftDragLeave() {
    focusDragOver.value = false;
  }

  async function onDraftDrop(e: DragEvent) {
    e.preventDefault();
    focusDragOver.value = false;
    const file = e.dataTransfer?.files?.[0];
    if (!file) return;
    if (!/\.txt$/i.test(file.name)) {
      focusDraftError.value = "Поддерживаются только .txt файлы";
      return;
    }
    try {
      const text = await file.text();
      focusDraftDomains.value = focusDraftDomains.value
        ? `${focusDraftDomains.value}\n${text}`
        : text;
      if (!focusDraftName.value) {
        focusDraftName.value = file.name.replace(/\.txt$/i, "");
      }
    } catch (err) {
      focusDraftError.value = `Не удалось прочитать файл: ${(err as Error).message}`;
    }
  }

  return {
    focusBlocklists,
    focusLoading,
    focusError,
    focusBackendMissing,
    focusActive,
    focusActiveBlocklist,
    focusBusy,
    focusServiceStatus,
    focusServiceBusy,
    focusServiceError,
    focusEditing,
    focusEditingId,
    focusDraftName,
    focusDraftDomains,
    focusDraftIcon,
    focusDraftError,
    focusDragOver,
    focusDraftParsed,
    mentionOpen,
    mentionCandidates,
    mentionTextareaRef,
    mentionHighlight,
    loadBlocklists,
    loadActiveState,
    refreshFocusServiceStatus,
    installFocusService,
    uninstallFocusService,
    openCreateBlocklist,
    openEditBlocklist,
    cancelCreateBlocklist,
    onCreateBlocklist,
    onDomainsInput,
    insertMention,
    onMentionKey,
    onDeleteBlocklist,
    onDeactivate,
    onActivate,
    onDraftDragOver,
    onDraftDragLeave,
    onDraftDrop,
  };
}

export type FocusTabCtx = ReturnType<typeof useFocusTab>;

export const FocusTabKey: InjectionKey<FocusTabCtx> = Symbol("FocusTabKey");
