// useFileSearchTab — state + handlers File Search tab'а: roots, ignore patterns,
// diagnostics, root estimation, noise/gitignore/hidden/NTFS toggles, rescan +
// scan-progress toasts.
//
// Composable принимает `toast` api (от parent's `provideToastHost`), так как
// прогресс индексации показывается toast'ом, который должен жить дольше mount'а
// этого таба.

import { computed, ref } from "vue";
import type {
  FileIndexSettings,
  FileSearchDiagnosticsReport,
  FileSearchRootWarning,
} from "@shared/ipc-types";
import type { ToastApi } from "@kosmos/visuals";

interface FileSearchPatch {
  exclude_noisy_folders?: boolean;
  respect_gitignore?: boolean;
  include_hidden?: boolean;
  ntfs_accelerated?: boolean;
}

function formatCount(value: number): string {
  return new Intl.NumberFormat("ru-RU").format(value);
}

function shortenPath(path: string): string {
  if (path.length <= 42) return path;
  return `${path.slice(0, 18)}…${path.slice(-20)}`;
}

function describeFileSearchError(err: unknown, fallback: string): string {
  const raw = (err as { message?: string } | null)?.message ?? String(err ?? "");
  const trimmed = raw.trim();
  if (!trimmed) return fallback;
  const known = [
    "pattern уже есть",
    "invalid ignore pattern",
    "must not be empty",
    "must be an existing directory",
  ];
  for (const marker of known) {
    if (trimmed.toLowerCase().includes(marker.toLowerCase())) {
      return trimmed.replace(/^[a-z_]+\.[a-z_]+:\s*/i, "");
    }
  }
  return `${fallback}: ${trimmed}`;
}

export function useFileSearchTab(toast: ToastApi) {
  const fileSearchSettings = ref<FileIndexSettings | null>(null);
  const fileSearchDiagnostics = ref<FileSearchDiagnosticsReport | null>(null);
  const fileSearchBusy = ref<boolean>(false);
  const fileSearchError = ref<string>("");
  const fileSearchNewIgnore = ref<string>("");
  let fileSearchPollTimer: number | null = null;
  let fileSearchToastId: number | null = null;

  const fileSearchRootWarnings = computed<FileSearchRootWarning[]>(() => {
    const diagnostics = fileSearchDiagnostics.value;
    if (!diagnostics || diagnostics.risk_level === "ok" || diagnostics.risk_reasons.length === 0) {
      return [];
    }
    return diagnostics.roots.map((root) => ({
      path: root,
      risk_level: diagnostics.risk_level === "danger" ? "danger" : "warning",
      risk_reasons: diagnostics.risk_reasons,
    }));
  });
  function formatFileSearchProgress(settings: FileIndexSettings | null): string {
    const progress = settings?.scan_progress;
    if (!progress) return "Ожидаем статус индексатора.";
    const parts: string[] = [];
    if (progress.root) {
      parts.push(shortenPath(progress.root));
    }
    if (progress.roots_total > 0) {
      parts.push(
        `папка ${Math.min(progress.roots_done + 1, progress.roots_total)}/${progress.roots_total}`,
      );
    }
    if (progress.files_seen > 0 || progress.files_indexed > 0) {
      parts.push(`${formatCount(progress.files_indexed || progress.files_seen)} файлов`);
    }
    if (progress.phase === "ntfs") {
      parts.push("NTFS scan");
    }
    return parts.length > 0 ? parts.join(" · ") : "Индексатор готовится.";
  }

  function applyFileSearchSettings(settings: FileIndexSettings) {
    fileSearchSettings.value = settings;
  }

  async function loadFileSearchSettings() {
    fileSearchError.value = "";
    try {
      applyFileSearchSettings(await window.kepler.fileSearch.settingsGet());
    } catch (err) {
      console.warn("file_index.settings_get failed", err);
      fileSearchError.value = "Настройки поиска файлов пока недоступны";
    }
  }

  async function loadFileSearchDiagnostics() {
    try {
      fileSearchDiagnostics.value = await window.kepler.fileSearch.diagnostics();
    } catch (err) {
      console.warn("file_index.diagnostics failed", err);
      fileSearchDiagnostics.value = null;
    }
  }

  async function loadFileSearchState() {
    await loadFileSearchSettings();
    void loadFileSearchDiagnostics();
    watchFileSearchProgressInline();
  }

  function clearFileSearchPoll() {
    if (fileSearchPollTimer) {
      window.clearTimeout(fileSearchPollTimer);
      fileSearchPollTimer = null;
    }
    if (fileSearchToastId !== null) {
      toast.dismiss(fileSearchToastId);
      fileSearchToastId = null;
    }
  }

  function watchFileSearchProgress(message = "Индексация файлов запущена") {
    clearFileSearchPoll();
    fileSearchToastId = toast.show({
      title: "Поиск файлов",
      message,
      description: formatFileSearchProgress(fileSearchSettings.value),
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
          toast.update(fileSearchToastId, {
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
            toast.update(fileSearchToastId, {
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
            toast.update(fileSearchToastId, {
              title: "Поиск файлов",
              message: "Индексация продолжается в фоне",
              description: "Статус обновится при следующем открытии настроек.",
              loading: false,
              closable: true,
              duration: 4200,
            });
            fileSearchToastId = null;
          } else {
            toast.show({
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
  }

  function watchFileSearchProgressInline() {
    if (fileSearchPollTimer || !fileSearchSettings.value?.scan_in_progress) return;
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
  }

  function showFileSearchBusyToast(message: string, description?: string): number {
    clearFileSearchPoll();
    fileSearchToastId = toast.show({
      title: "Поиск файлов",
      message,
      description: description ?? formatFileSearchProgress(fileSearchSettings.value),
      tone: "info",
      duration: 0,
      loading: true,
      closable: true,
    });
    return fileSearchToastId;
  }

  function finishFileSearchBusyToast(
    id: number,
    message: string,
    tone: "success" | "info" = "success",
    description?: string,
  ) {
    if (fileSearchToastId !== id) return;
    toast.update(id, {
      title: "Поиск файлов",
      message,
      description: description ?? formatFileSearchProgress(fileSearchSettings.value),
      tone,
      duration: 3200,
      loading: false,
      closable: true,
    });
    fileSearchToastId = null;
  }

  async function updateFileSearchSettings(patch: FileSearchPatch) {
    const snapshot = fileSearchSettings.value ? { ...fileSearchSettings.value } : null;
    if (fileSearchSettings.value) {
      fileSearchSettings.value = { ...fileSearchSettings.value, ...patch };
    }
    fileSearchBusy.value = true;
    fileSearchError.value = "";
    try {
      await window.kepler.fileSearch.settingsSet(patch);
      await loadFileSearchState();
      watchFileSearchProgress("Настройка сохранена, индекс обновляется");
    } catch (err) {
      console.warn("file_index.settings_set failed", err);
      if (snapshot) fileSearchSettings.value = snapshot;
      fileSearchError.value = describeFileSearchError(err, "Не удалось применить настройку");
      await loadFileSearchState();
    } finally {
      fileSearchBusy.value = false;
    }
  }

  async function onToggleFileSearchNoise(e: Event) {
    await updateFileSearchSettings({
      exclude_noisy_folders: (e.target as HTMLInputElement).checked,
    });
  }

  async function onToggleFileSearchGitignore(e: Event) {
    await updateFileSearchSettings({
      respect_gitignore: (e.target as HTMLInputElement).checked,
    });
  }

  async function onToggleFileSearchHidden(e: Event) {
    await updateFileSearchSettings({
      include_hidden: (e.target as HTMLInputElement).checked,
    });
  }

  async function onToggleFileSearchNtfs(e: Event) {
    await updateFileSearchSettings({
      ntfs_accelerated: (e.target as HTMLInputElement).checked,
    });
  }

  async function onAddFileSearchScope() {
    fileSearchError.value = "";
    const picked = await window.kepler.fileSearch.pickScope();
    if (!picked) return;
    fileSearchBusy.value = true;
    const toastId = showFileSearchBusyToast("Добавляем папку поиска", shortenPath(picked));
    try {
      await window.kepler.fileSearch.scopeAdd(picked);
      await loadFileSearchState();
      watchFileSearchProgress("Папка добавлена, индексация запущена");
    } catch (err) {
      console.warn("file_index.scope_add failed", err);
      fileSearchError.value = describeFileSearchError(err, "Не удалось добавить папку поиска");
      finishFileSearchBusyToast(toastId, "Не удалось добавить папку", "info");
    } finally {
      fileSearchBusy.value = false;
    }
  }

  async function onRemoveFileSearchScope(path: string) {
    const confirmed = window.confirm(
      `Удалить папку поиска «${path}»? Все её проиндексированные файлы будут удалены.`,
    );
    if (!confirmed) return;
    fileSearchBusy.value = true;
    fileSearchError.value = "";
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
      fileSearchError.value = describeFileSearchError(err, "Не удалось удалить папку поиска");
      finishFileSearchBusyToast(toastId, "Не удалось удалить папку", "info");
    } finally {
      fileSearchBusy.value = false;
    }
  }

  async function onAddFileSearchIgnore() {
    const pattern = fileSearchNewIgnore.value.trim();
    if (!pattern) return;
    const existing = fileSearchSettings.value?.ignore_patterns ?? [];
    if (existing.some((p) => p.toLowerCase() === pattern.toLowerCase())) {
      fileSearchError.value = `Шаблон уже добавлен: ${pattern}`;
      return;
    }
    fileSearchBusy.value = true;
    fileSearchError.value = "";
    const toastId = showFileSearchBusyToast("Добавляем шаблон исключения", pattern);
    try {
      await window.kepler.fileSearch.ignoreAdd(pattern);
      fileSearchNewIgnore.value = "";
      await loadFileSearchState();
      watchFileSearchProgress("Шаблон добавлен, индекс обновляется");
    } catch (err) {
      console.warn("file_index.ignore_add failed", err);
      fileSearchError.value = describeFileSearchError(err, "Не удалось добавить шаблон");
      finishFileSearchBusyToast(toastId, "Не удалось добавить шаблон", "info");
    } finally {
      fileSearchBusy.value = false;
    }
  }

  async function onRemoveFileSearchIgnore(pattern: string) {
    fileSearchBusy.value = true;
    fileSearchError.value = "";
    const toastId = showFileSearchBusyToast("Удаляем шаблон исключения", pattern);
    try {
      await window.kepler.fileSearch.ignoreRemove(pattern);
      await loadFileSearchState();
      watchFileSearchProgress("Шаблон удалён, индекс обновляется");
    } catch (err) {
      console.warn("file_index.ignore_remove failed", err);
      fileSearchError.value = describeFileSearchError(err, "Не удалось удалить шаблон");
      finishFileSearchBusyToast(toastId, "Не удалось удалить шаблон", "info");
    } finally {
      fileSearchBusy.value = false;
    }
  }

  async function onRescanFileSearch() {
    if (fileSearchSettings.value?.enabled === false) {
      fileSearchError.value = "Поиск файлов выключен";
      return;
    }
    if (fileSearchSettings.value?.scan_in_progress) {
      fileSearchError.value = "Индексация уже идёт";
      return;
    }
    fileSearchBusy.value = true;
    fileSearchError.value = "";
    const toastId = showFileSearchBusyToast("Запускаем переиндексацию");
    try {
      await window.kepler.fileSearch.rescan();
      await loadFileSearchState();
      watchFileSearchProgress("Переиндексация запущена");
    } catch (err) {
      console.warn("file_index.rescan failed", err);
      fileSearchError.value = describeFileSearchError(err, "Не удалось переиндексировать файлы");
      finishFileSearchBusyToast(toastId, "Не удалось запустить переиндексацию", "info");
    } finally {
      fileSearchBusy.value = false;
    }
  }

  async function onClearFileSearchCache() {
    const confirmed = window.confirm(
      "Очистить кеш поиска файлов? Папки поиска и настройки сохранятся, но результаты исчезнут до следующей переиндексации.",
    );
    if (!confirmed) return;
    fileSearchBusy.value = true;
    fileSearchError.value = "";
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
      fileSearchError.value = describeFileSearchError(err, "Не удалось очистить индекс");
      finishFileSearchBusyToast(toastId, "Не удалось очистить индекс", "info");
    } finally {
      fileSearchBusy.value = false;
    }
  }

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
