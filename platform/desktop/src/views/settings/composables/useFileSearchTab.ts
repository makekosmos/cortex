// useFileSearchTab — state + handlers File Search tab'а: roots, ignore patterns,
// noise/gitignore/hidden/NTFS toggles, rescan + scan-progress toasts.
//
// Composable принимает `toast` api (от parent's `provideToastHost`), так как
// прогресс индексации показывается toast'ом, который должен жить дольше mount'а
// этого таба.

import { ref } from "vue";
import type { FileIndexSettings } from "@shared/ipc-types";
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
  const fileSearchBusy = ref<boolean>(false);
  const fileSearchError = ref<string>("");
  const fileSearchNewIgnore = ref<string>("");
  let fileSearchPollTimer: number | null = null;
  let fileSearchToastId: number | null = null;

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

  async function loadFileSearchSettings() {
    fileSearchError.value = "";
    try {
      fileSearchSettings.value = await window.kepler.fileSearch.settingsGet();
    } catch (err) {
      console.warn("file_index.settings_get failed", err);
      fileSearchError.value = "Настройки поиска файлов пока недоступны";
    }
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
        fileSearchSettings.value = next;
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

  async function updateFileSearchSettings(patch: FileSearchPatch) {
    const snapshot = fileSearchSettings.value ? { ...fileSearchSettings.value } : null;
    if (fileSearchSettings.value) {
      fileSearchSettings.value = { ...fileSearchSettings.value, ...patch };
    }
    fileSearchBusy.value = true;
    fileSearchError.value = "";
    try {
      await window.kepler.fileSearch.settingsSet(patch);
      await loadFileSearchSettings();
      if (fileSearchSettings.value?.scan_in_progress) {
        watchFileSearchProgress("Настройка сохранена, индекс обновляется");
      }
    } catch (err) {
      console.warn("file_index.settings_set failed", err);
      if (snapshot) fileSearchSettings.value = snapshot;
      fileSearchError.value = describeFileSearchError(err, "Не удалось применить настройку");
      await loadFileSearchSettings();
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
    try {
      await window.kepler.fileSearch.scopeAdd(picked);
      await loadFileSearchSettings();
      watchFileSearchProgress("Папка добавлена, индексация запущена");
    } catch (err) {
      console.warn("file_index.scope_add failed", err);
      fileSearchError.value = "Не удалось добавить папку поиска";
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
    try {
      await window.kepler.fileSearch.scopeRemove(path);
      await loadFileSearchSettings();
      watchFileSearchProgress("Папка удалена, индекс обновляется");
    } catch (err) {
      console.warn("file_index.scope_remove failed", err);
      fileSearchError.value = describeFileSearchError(err, "Не удалось удалить папку поиска");
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
    try {
      await window.kepler.fileSearch.ignoreAdd(pattern);
      fileSearchNewIgnore.value = "";
      await loadFileSearchSettings();
      watchFileSearchProgress("Шаблон добавлен, индекс обновляется");
    } catch (err) {
      console.warn("file_index.ignore_add failed", err);
      fileSearchError.value = describeFileSearchError(err, "Не удалось добавить шаблон");
    } finally {
      fileSearchBusy.value = false;
    }
  }

  async function onRemoveFileSearchIgnore(pattern: string) {
    fileSearchBusy.value = true;
    fileSearchError.value = "";
    try {
      await window.kepler.fileSearch.ignoreRemove(pattern);
      await loadFileSearchSettings();
      watchFileSearchProgress("Шаблон удалён, индекс обновляется");
    } catch (err) {
      console.warn("file_index.ignore_remove failed", err);
      fileSearchError.value = describeFileSearchError(err, "Не удалось удалить шаблон");
    } finally {
      fileSearchBusy.value = false;
    }
  }

  async function onRescanFileSearch() {
    if (fileSearchSettings.value?.scan_in_progress) {
      fileSearchError.value = "Индексация уже идёт";
      return;
    }
    fileSearchBusy.value = true;
    fileSearchError.value = "";
    try {
      await window.kepler.fileSearch.rescan();
      await loadFileSearchSettings();
      watchFileSearchProgress("Переиндексация запущена");
    } catch (err) {
      console.warn("file_index.rescan failed", err);
      fileSearchError.value = describeFileSearchError(err, "Не удалось переиндексировать файлы");
    } finally {
      fileSearchBusy.value = false;
    }
  }

  return {
    fileSearchSettings,
    fileSearchBusy,
    fileSearchError,
    fileSearchNewIgnore,
    loadFileSearchSettings,
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
  };
}
