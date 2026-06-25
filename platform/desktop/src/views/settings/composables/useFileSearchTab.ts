import { computed, ref } from "vue";
import type {
  FileIndexSettings,
  FileSearchDiagnosticsReport,
  FileSearchRootWarning,
} from "@shared/ipc-types";
import type { ToastApi } from "@kosmos/visuals";
import { buildFileSearchRootWarnings } from "./useFileSearchTab.shared";
import { createFileSearchTabActions } from "./useFileSearchTab.actions";

export function useFileSearchTab(toast: ToastApi) {
  const fileSearchSettings = ref<FileIndexSettings | null>(null);
  const fileSearchDiagnostics = ref<FileSearchDiagnosticsReport | null>(null);
  const fileSearchBusy = ref<boolean>(false);
  const fileSearchError = ref<string>("");
  const fileSearchNewIgnore = ref<string>("");

  const fileSearchRootWarnings = computed<FileSearchRootWarning[]>(() =>
    buildFileSearchRootWarnings(fileSearchDiagnostics.value),
  );

  const {
    loadFileSearchSettings,
    loadFileSearchDiagnostics,
    loadFileSearchState,
    clearFileSearchPoll,
    onToggleFileSearchNoise,
    onToggleFileSearchGitignore,
    onToggleFileSearchHidden,
    onToggleFileSearchNtfs,
    onAddFileSearchScope,
    onRemoveFileSearchScope,
    onAddFileSearchIgnore,
    onRemoveFileSearchIgnore,
    onRescanFileSearch,
    onClearFileSearchCache,
  } = createFileSearchTabActions({
    toast,
    fileSearchSettings,
    fileSearchDiagnostics,
    fileSearchBusy,
    fileSearchError,
    fileSearchNewIgnore,
  });

  return {
    fileSearchSettings,
    fileSearchDiagnostics,
    fileSearchBusy,
    fileSearchError,
    fileSearchNewIgnore,
    loadFileSearchSettings,
    loadFileSearchDiagnostics,
    loadFileSearchState,
    clearFileSearchPoll,
    fileSearchRootWarnings,
    onToggleFileSearchNoise,
    onToggleFileSearchGitignore,
    onToggleFileSearchHidden,
    onToggleFileSearchNtfs,
    onAddFileSearchScope,
    onRemoveFileSearchScope,
    onAddFileSearchIgnore,
    onRemoveFileSearchIgnore,
    onRescanFileSearch,
    onClearFileSearchCache,
  };
}
