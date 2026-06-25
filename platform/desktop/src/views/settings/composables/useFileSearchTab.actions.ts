import type { Ref } from "vue";
import type { ToastApi } from "@kosmos/visuals";
import type { FileIndexSettings, FileSearchDiagnosticsReport } from "@shared/ipc-types";
import { shortenPath } from "./useFileSearchTab.shared";
import { createFileSearchProgressActions } from "./useFileSearchTab.progress";

interface FileSearchTabRuntime {
  toast: ToastApi;
  fileSearchSettings: Ref<FileIndexSettings | null>;
  fileSearchDiagnostics: Ref<FileSearchDiagnosticsReport | null>;
  fileSearchBusy: Ref<boolean>;
  fileSearchError: Ref<string>;
  fileSearchNewIgnore: Ref<string>;
}

export function createFileSearchTabActions(runtime: FileSearchTabRuntime) {
  const {
    loadFileSearchSettings,
    loadFileSearchDiagnostics,
    loadFileSearchState,
    clearFileSearchPoll,
    watchFileSearchProgress,
    showFileSearchBusyToast,
    finishFileSearchBusyToast,
    updateFileSearchSettings,
  } = createFileSearchProgressActions({
    toast: runtime.toast,
    fileSearchSettings: runtime.fileSearchSettings,
    fileSearchDiagnostics: runtime.fileSearchDiagnostics,
    fileSearchBusy: runtime.fileSearchBusy,
    fileSearchError: runtime.fileSearchError,
  });

  const onToggleFileSearchNoise = async (e: Event) => {
    await updateFileSearchSettings({
      exclude_noisy_folders: (e.target as HTMLInputElement).checked,
    });
  };

  const onToggleFileSearchGitignore = async (e: Event) => {
    await updateFileSearchSettings({
      respect_gitignore: (e.target as HTMLInputElement).checked,
    });
  };

  const onToggleFileSearchHidden = async (e: Event) => {
    await updateFileSearchSettings({
      include_hidden: (e.target as HTMLInputElement).checked,
    });
  };

  const onToggleFileSearchNtfs = async (e: Event) => {
    await updateFileSearchSettings({
      ntfs_accelerated: (e.target as HTMLInputElement).checked,
    });
  };

  const onAddFileSearchScope = async () => {
    runtime.fileSearchError.value = "";
    const picked = await window.kepler.fileSearch.pickScope();
    if (!picked) return;
    runtime.fileSearchBusy.value = true;
    const toastId = showFileSearchBusyToast("Добавляем папку поиска", shortenPath(picked));
    try {
      await window.kepler.fileSearch.scopeAdd(picked);
      await loadFileSearchState();
      watchFileSearchProgress("Папка добавлена, индексация запущена");
    } catch (err) {
      console.warn("file_index.scope_add failed", err);
      runtime.fileSearchError.value = `Не удалось добавить папку поиска: ${
        (err as { message?: string } | null)?.message ?? String(err ?? "")
      }`;
      finishFileSearchBusyToast(toastId, "Не удалось добавить папку", "info");
    } finally {
      runtime.fileSearchBusy.value = false;
    }
  };

  const onRemoveFileSearchScope = async (path: string) => {
    const confirmed = window.confirm(
      `Удалить папку поиска «${path}»? Все её проиндексированные файлы будут удалены.`,
    );
    if (!confirmed) return;
    runtime.fileSearchBusy.value = true;
    runtime.fileSearchError.value = "";
    const toastId = showFileSearchBusyToast(
      "Удаляем папку из индекса",
      `${shortenPath(path)} · очистка кеша продолжится в фоне`,
    );
    try {
      await window.kepler.fileSearch.scopeRemove(path);
      await loadFileSearchState();
      watchFileSearchProgress("Папка удалена, индекс обновляется");
    } catch (err) {
      console.warn("file_index.scope_remove failed", err);
      runtime.fileSearchError.value = `Не удалось удалить папку поиска: ${
        (err as { message?: string } | null)?.message ?? String(err ?? "")
      }`;
      finishFileSearchBusyToast(toastId, "Не удалось удалить папку", "info");
    } finally {
      runtime.fileSearchBusy.value = false;
    }
  };

  const onAddFileSearchIgnore = async () => {
    const pattern = runtime.fileSearchNewIgnore.value.trim();
    if (!pattern) return;
    const existing = Array.isArray(runtime.fileSearchSettings.value?.ignore_patterns)
      ? runtime.fileSearchSettings.value.ignore_patterns
      : [];
    if (existing.some((p) => p.toLowerCase() === pattern.toLowerCase())) {
      runtime.fileSearchError.value = `Шаблон уже добавлен: ${pattern}`;
      return;
    }
    runtime.fileSearchBusy.value = true;
    runtime.fileSearchError.value = "";
    const toastId = showFileSearchBusyToast("Добавляем шаблон исключения", pattern);
    try {
      await window.kepler.fileSearch.ignoreAdd(pattern);
      runtime.fileSearchNewIgnore.value = "";
      await loadFileSearchState();
      watchFileSearchProgress("Шаблон добавлен, индекс обновляется");
    } catch (err) {
      console.warn("file_index.ignore_add failed", err);
      runtime.fileSearchError.value = `Не удалось добавить шаблон: ${
        (err as { message?: string } | null)?.message ?? String(err ?? "")
      }`;
      finishFileSearchBusyToast(toastId, "Не удалось добавить шаблон", "info");
    } finally {
      runtime.fileSearchBusy.value = false;
    }
  };

  const onRemoveFileSearchIgnore = async (pattern: string) => {
    runtime.fileSearchBusy.value = true;
    runtime.fileSearchError.value = "";
    const toastId = showFileSearchBusyToast("Удаляем шаблон исключения", pattern);
    try {
      await window.kepler.fileSearch.ignoreRemove(pattern);
      await loadFileSearchState();
      watchFileSearchProgress("Шаблон удалён, индекс обновляется");
    } catch (err) {
      console.warn("file_index.ignore_remove failed", err);
      runtime.fileSearchError.value = `Не удалось удалить шаблон: ${
        (err as { message?: string } | null)?.message ?? String(err ?? "")
      }`;
      finishFileSearchBusyToast(toastId, "Не удалось удалить шаблон", "info");
    } finally {
      runtime.fileSearchBusy.value = false;
    }
  };

  const onRescanFileSearch = async () => {
    if (runtime.fileSearchSettings.value?.enabled === false) {
      runtime.fileSearchError.value = "Поиск файлов выключен";
      return;
    }
    if (runtime.fileSearchSettings.value?.scan_in_progress) {
      runtime.fileSearchError.value = "Индексация уже идёт";
      return;
    }
    runtime.fileSearchBusy.value = true;
    runtime.fileSearchError.value = "";
    const toastId = showFileSearchBusyToast("Запускаем переиндексацию");
    try {
      await window.kepler.fileSearch.rescan();
      await loadFileSearchState();
      watchFileSearchProgress("Переиндексация запущена");
    } catch (err) {
      console.warn("file_index.rescan failed", err);
      runtime.fileSearchError.value = `Не удалось переиндексировать файлы: ${
        (err as { message?: string } | null)?.message ?? String(err ?? "")
      }`;
      finishFileSearchBusyToast(toastId, "Не удалось запустить переиндексацию", "info");
    } finally {
      runtime.fileSearchBusy.value = false;
    }
  };

  const onClearFileSearchCache = async () => {
    const confirmed = window.confirm(
      "Очистить кеш поиска файлов? Папки поиска и настройки сохранятся, но результаты исчезнут до следующей переиндексации.",
    );
    if (!confirmed) return;
    runtime.fileSearchBusy.value = true;
    runtime.fileSearchError.value = "";
    const toastId = showFileSearchBusyToast(
      "Очищаем индекс",
      "Папки поиска и настройки сохранятся.",
    );
    try {
      await window.kepler.fileSearch.clearCache();
      await loadFileSearchState();
      finishFileSearchBusyToast(
        toastId,
        "Индекс очищен",
        "success",
        "Запусти переиндексацию вручную.",
      );
    } catch (err) {
      console.warn("file_index.clear_cache failed", err);
      runtime.fileSearchError.value = `Не удалось очистить индекс: ${
        (err as { message?: string } | null)?.message ?? String(err ?? "")
      }`;
      finishFileSearchBusyToast(toastId, "Не удалось очистить индекс", "info");
    } finally {
      runtime.fileSearchBusy.value = false;
    }
  };

  return {
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
  };
}
