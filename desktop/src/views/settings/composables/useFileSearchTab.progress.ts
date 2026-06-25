import type { Ref } from "vue";
import type { ToastApi } from "@kosmos/visuals";
import type { FileIndexSettings, FileSearchDiagnosticsReport } from "@shared/ipc-types";
import {
  describeFileSearchError,
  formatFileSearchProgress,
  type FileSearchPatch,
} from "./useFileSearchTab.shared";

interface FileSearchTabRuntime {
  toast: ToastApi;
  fileSearchSettings: Ref<FileIndexSettings | null>;
  fileSearchDiagnostics: Ref<FileSearchDiagnosticsReport | null>;
  fileSearchBusy: Ref<boolean>;
  fileSearchError: Ref<string>;
}

export function createFileSearchProgressActions(runtime: FileSearchTabRuntime) {
  let fileSearchPollTimer: number | null = null;
  let fileSearchToastId: number | null = null;

  const applyFileSearchSettings = (settings: FileIndexSettings) => {
    runtime.fileSearchSettings.value = settings;
  };

  const loadFileSearchSettings = async () => {
    runtime.fileSearchError.value = "";
    try {
      applyFileSearchSettings(await window.kepler.fileSearch.settingsGet());
    } catch (err) {
      console.warn("file_index.settings_get failed", err);
      runtime.fileSearchError.value = "Настройки поиска файлов пока недоступны";
    }
  };

  const loadFileSearchDiagnostics = async () => {
    try {
      runtime.fileSearchDiagnostics.value = await window.kepler.fileSearch.diagnostics();
    } catch (err) {
      console.warn("file_index.diagnostics failed", err);
      runtime.fileSearchDiagnostics.value = null;
    }
  };

  const clearFileSearchPoll = () => {
    if (fileSearchPollTimer) {
      window.clearTimeout(fileSearchPollTimer);
      fileSearchPollTimer = null;
    }
    if (fileSearchToastId !== null) {
      runtime.toast.dismiss(fileSearchToastId);
      fileSearchToastId = null;
    }
  };

  const watchFileSearchProgress = (message = "Индексация файлов запущена") => {
    clearFileSearchPoll();
    fileSearchToastId = runtime.toast.show({
      title: "Поиск файлов",
      message,
      description: formatFileSearchProgress(runtime.fileSearchSettings.value),
      tone: "info",
      duration: 0,
      loading: true,
      closable: true,
    });
    const startedAt = Date.now();
    const poll = async () => {
      try {
        const next = await window.kepler.fileSearch.settingsGet();
        applyFileSearchSettings(next);
        if (fileSearchToastId !== null) {
          runtime.toast.update(fileSearchToastId, {
            title: "Индексируем файлы",
            message: next.scan_progress.message || "Индексация файлов",
            description: formatFileSearchProgress(next),
            tone: "info",
            loading: true,
            duration: 0,
            closable: true,
          });
        }
        if (!next.scan_in_progress) {
          void loadFileSearchDiagnostics();
          if (fileSearchToastId !== null) {
            runtime.toast.update(fileSearchToastId, {
              title: "Поиск файлов",
              message: "Индексация завершена",
              description: formatFileSearchProgress(next),
              tone: "success",
              loading: false,
              duration: 2600,
              closable: true,
            });
            fileSearchToastId = null;
          }
          fileSearchPollTimer = null;
          return;
        }
      } catch (err) {
        console.warn("file_index progress poll failed", err);
        if (Date.now() - startedAt > 90_000) {
          if (fileSearchToastId !== null) {
            runtime.toast.update(fileSearchToastId, {
              title: "Поиск файлов",
              message: "Индексация продолжается в фоне",
              description: "Статус обновится при следующем открытии настроек.",
              loading: false,
              closable: true,
              duration: 4200,
            });
            fileSearchToastId = null;
          } else {
            runtime.toast.show({
              title: "Поиск файлов",
              message: "Индексация продолжается в фоне, статус обновится позже",
              tone: "info",
              duration: 3200,
            });
          }
          fileSearchPollTimer = null;
          return;
        }
      }
      fileSearchPollTimer = window.setTimeout(poll, 2000);
    };
    fileSearchPollTimer = window.setTimeout(poll, 1200);
  };

  const watchFileSearchProgressInline = () => {
    if (fileSearchPollTimer || !runtime.fileSearchSettings.value?.scan_in_progress) return;
    const poll = async () => {
      try {
        const next = await window.kepler.fileSearch.settingsGet();
        applyFileSearchSettings(next);
        if (!next.scan_in_progress) {
          void loadFileSearchDiagnostics();
          fileSearchPollTimer = null;
          return;
        }
      } catch (err) {
        console.warn("file_index inline progress poll failed", err);
      }
      fileSearchPollTimer = window.setTimeout(poll, 2000);
    };
    fileSearchPollTimer = window.setTimeout(poll, 1000);
  };

  const showFileSearchBusyToast = (message: string, description?: string): number => {
    clearFileSearchPoll();
    fileSearchToastId = runtime.toast.show({
      title: "Поиск файлов",
      message,
      description: description ?? formatFileSearchProgress(runtime.fileSearchSettings.value),
      tone: "info",
      duration: 0,
      loading: true,
      closable: true,
    });
    return fileSearchToastId;
  };

  const finishFileSearchBusyToast = (
    id: number,
    message: string,
    tone: "success" | "info" = "success",
    description?: string,
  ) => {
    if (fileSearchToastId !== id) return;
    runtime.toast.update(id, {
      title: "Поиск файлов",
      message,
      description: description ?? formatFileSearchProgress(runtime.fileSearchSettings.value),
      tone,
      duration: 3200,
      loading: false,
      closable: true,
    });
    fileSearchToastId = null;
  };

  const loadFileSearchState = async () => {
    await loadFileSearchSettings();
    void loadFileSearchDiagnostics();
    watchFileSearchProgressInline();
  };

  const updateFileSearchSettings = async (patch: FileSearchPatch) => {
    const snapshot = runtime.fileSearchSettings.value
      ? { ...runtime.fileSearchSettings.value }
      : null;
    if (runtime.fileSearchSettings.value) {
      runtime.fileSearchSettings.value = { ...runtime.fileSearchSettings.value, ...patch };
    }
    runtime.fileSearchBusy.value = true;
    runtime.fileSearchError.value = "";
    try {
      await window.kepler.fileSearch.settingsSet(patch);
      await loadFileSearchState();
      watchFileSearchProgress("Настройка сохранена, индекс обновляется");
    } catch (err) {
      console.warn("file_index.settings_set failed", err);
      if (snapshot) runtime.fileSearchSettings.value = snapshot;
      runtime.fileSearchError.value = describeFileSearchError(
        err,
        "Не удалось применить настройку",
      );
      await loadFileSearchState();
    } finally {
      runtime.fileSearchBusy.value = false;
    }
  };

  return {
    loadFileSearchSettings,
    loadFileSearchDiagnostics,
    loadFileSearchState,
    clearFileSearchPoll,
    watchFileSearchProgress,
    showFileSearchBusyToast,
    finishFileSearchBusyToast,
    updateFileSearchSettings,
  };
}
