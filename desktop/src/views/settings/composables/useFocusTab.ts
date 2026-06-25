import { computed, ref, type InjectionKey } from "vue";
import {
  isUnknownOperationError,
  type FocusActiveState,
  type FocusBlocklist,
} from "./useFocusTab.shared";
import { useFocusTabDraft } from "./useFocusTabDraft";
export type { FocusActiveState, FocusBlocklist } from "./useFocusTab.shared";
export { ICON_CHOICES } from "./useFocusTab.shared";

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
      focusBlocklists.value = Array.isArray(r?.blocklists) ? r.blocklists : [];
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

  const focusActiveBlocklist = computed<FocusBlocklist | undefined>(() => {
    if (!focusActive.value.active || !focusActive.value.blocklist_id) return undefined;
    return focusBlocklists.value.find((b) => b.id === focusActive.value.blocklist_id);
  });

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

  const draft = useFocusTabDraft({
    focusBusy,
    focusEditing: ref(false),
    focusEditingId: ref<string | null>(null),
    focusDraftName: ref<string>(""),
    focusDraftDomains: ref<string>(""),
    focusDraftIcon: ref<string>(""),
    focusDraftError: ref<string>(""),
    focusDragOver: ref<boolean>(false),
    focusBlocklists,
    mentionOpen: ref<boolean>(false),
    mentionQuery: ref<string>(""),
    mentionAnchor: ref<number>(0),
    mentionTextareaRef: ref<HTMLTextAreaElement | null>(null),
    mentionHighlight: ref<number>(0),
    focusRequest,
    loadBlocklists,
  });

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
    ...draft,
    loadBlocklists,
    loadActiveState,
    refreshFocusServiceStatus,
    installFocusService,
    uninstallFocusService,
    onDeleteBlocklist,
    onDeactivate,
    onActivate,
  };
}

export type FocusTabCtx = ReturnType<typeof useFocusTab>;

export const FocusTabKey: InjectionKey<FocusTabCtx> = Symbol("FocusTabKey");
