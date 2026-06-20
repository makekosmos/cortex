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
  FileSearchRootEstimate,
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

function formatBytes(value: number): string {
  if (!Number.isFinite(value) || value <= 0) return "0 Б";
  const units = ["Б", "КБ", "МБ", "ГБ", "ТБ"];
  let current = value;
  let unit = 0;
  while (current >= 1024 && unit < units.length - 1) {
    current /= 1024;
    unit += 1;
  }
  const digits = unit === 0 || current >= 100 ? 0 : current >= 10 ? 1 : 2;
  return `${current.toFixed(digits)} ${units[unit]}`;
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
  const fileSearchRootModalOpen = ref<boolean>(false);
  const fileSearchRootEstimate = ref<FileSearchRootEstimate | null>(null);
  const fileSearchRootEstimateLoading = ref<boolean>(false);
  const fileSearchRootEstimateError = ref<string>("");
  const fileSearchRootPendingPath = ref<string>("");
  const fileSearchProgressLastChangedAt = ref<number>(Date.now());
  const fileSearchProgressNow = ref<number>(Date.now());
  let fileSearchPollTimer: number | null = null;
  let fileSearchToastId: number | null = null;
  let fileSearchProgressSignature = "";

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
  const fileSearchIndexTotalBytes = computed(
    () => fileSearchDiagnostics.value?.total_size_bytes ?? 0,
  );
  const fileSearchIndexFilesCount = computed(() => fileSearchDiagnostics.value?.files_count ?? 0);
  const fileSearchIndexTotalBytesLabel = computed(() =>
    fileSearchDiagnostics.value ? formatBytes(fileSearchIndexTotalBytes.value) : "—",
  );
  const fileSearchIndexFilesCountLabel = computed(() =>
    fileSearchDiagnostics.value ? formatCount(fileSearchIndexFilesCount.value) : "—",
  );
  const fileSearchScanStateLabel = computed(() => {
    if (fileSearchSettings.value?.enabled === false) {
      return "Выключен";
    }
    if (
      fileSearchDiagnostics.value?.scan_in_progress ||
      fileSearchSettings.value?.scan_in_progress
    ) {
      return "Идёт сканирование";
    }
    return "Готов";
  });
  const fileSearchProgressVisible = computed(() => {
    const settings = fileSearchSettings.value;
    return Boolean(settings?.enabled !== false && settings?.scan_in_progress);
  });
  const fileSearchProgressTitle = computed(() => {
    const progress = fileSearchSettings.value?.scan_progress;
    return progress?.message || "Индексируем файлы";
  });
  const fileSearchProgressPercent = computed(() => {
    const progress = fileSearchSettings.value?.scan_progress;
    if (!progress || progress.roots_total <= 0) return 0;
    return Math.max(0, Math.min(100, (progress.roots_done / progress.roots_total) * 100));
  });
  const fileSearchProgressDetails = computed(() =>
    formatFileSearchProgress(fileSearchSettings.value),
  );
  const fileSearchProgressStallLabel = computed(() => {
    if (!fileSearchProgressVisible.value) return "";
    const elapsedSec = Math.floor(
      Math.max(0, fileSearchProgressNow.value - fileSearchProgressLastChangedAt.value) / 1000,
    );
    if (elapsedSec < 8) return "обновляется";
    if (elapsedSec < 30) return `без изменений ${elapsedSec} с`;
    return `без изменений ${elapsedSec} с · возможно большой каталог`;
  });
  const fileSearchRootConfirmLabel = computed(() => {
    const estimate = fileSearchRootEstimate.value;
    if (!estimate) return "Добавить папку";
    return estimate.risk_level === "ok" ? "Добавить папку" : "Добавить всё равно";
  });
  const fileSearchRootConfirmTone = computed<"primary" | "danger">(() => {
    const estimate = fileSearchRootEstimate.value;
    return estimate && estimate.risk_level === "danger" ? "danger" : "primary";
  });
  const fileSearchRootEstimateSummary = computed(() => {
    const estimate = fileSearchRootEstimate.value;
    if (!estimate) return null;
    const mediaLimitation =
      estimate.metadata_only_media_files_count > 0
        ? "Медиа-файлы индексируются только по имени и пути, без чтения содержимого."
        : null;
    const limitations = Array.from(
      new Set(
        [...(estimate.limitations ?? []), mediaLimitation].filter((item): item is string => !!item),
      ),
    );
    const showTruncatedHint =
      estimate.truncated &&
      !estimate.risk_reasons.some((reason) => reason.toLowerCase().includes("усеч"));
    return {
      indexBytes: formatBytes(estimate.estimated_index_size_bytes),
      textFiles: formatCount(estimate.indexable_text_files_count),
      textBytes: formatBytes(estimate.indexable_text_bytes),
      mediaFiles: formatCount(estimate.metadata_only_media_files_count),
      otherFiles: formatCount(estimate.metadata_only_other_files_count),
      skippedFiles: formatCount(estimate.ignored_or_skipped_files),
      scannedEntries: formatCount(estimate.scanned_dirs + estimate.scanned_files),
      riskLevel: estimate.risk_level,
      isTruncated: estimate.truncated,
      showTruncatedHint,
      riskReasons: estimate.risk_reasons,
      limitations,
    };
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

  function rememberFileSearchProgress(settings: FileIndexSettings | null) {
    fileSearchProgressNow.value = Date.now();
    if (!settings?.scan_in_progress) {
      fileSearchProgressSignature = "";
      fileSearchProgressLastChangedAt.value = fileSearchProgressNow.value;
      return;
    }
    const progress = settings.scan_progress;
    const nextSignature = [
      progress.phase,
      progress.root ?? "",
      progress.roots_done,
      progress.roots_total,
      progress.files_seen,
      progress.files_indexed,
      progress.message,
    ].join("|");
    if (nextSignature !== fileSearchProgressSignature) {
      fileSearchProgressSignature = nextSignature;
      fileSearchProgressLastChangedAt.value = fileSearchProgressNow.value;
    }
  }

  function applyFileSearchSettings(settings: FileIndexSettings) {
    fileSearchSettings.value = settings;
    rememberFileSearchProgress(settings);
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

  function openFileSearchRootModal(path: string, estimate: FileSearchRootEstimate) {
    fileSearchRootPendingPath.value = path;
    fileSearchRootEstimate.value = estimate;
    fileSearchRootEstimateLoading.value = false;
    fileSearchRootModalOpen.value = true;
  }

  function closeFileSearchRootModal() {
    if (fileSearchRootEstimateLoading.value) return;
    fileSearchRootModalOpen.value = false;
    fileSearchRootPendingPath.value = "";
    fileSearchRootEstimate.value = null;
    fileSearchRootEstimateError.value = "";
  }

  async function prepareFileSearchRootAddition(path: string) {
    fileSearchRootEstimateLoading.value = true;
    fileSearchRootEstimateError.value = "";
    try {
      const estimate = await window.kepler.fileSearch.estimateRoot(path);
      openFileSearchRootModal(path, estimate);
    } catch (err) {
      console.warn("file_index.estimate_root failed", err);
      const errorMessage = describeFileSearchError(err, "Не удалось оценить корень");
      fileSearchRootEstimateError.value = errorMessage;
      openFileSearchRootModal(path, {
        path,
        scanned_dirs: 0,
        scanned_files: 0,
        ignored_or_skipped_files: 0,
        indexable_text_files_count: 0,
        indexable_text_bytes: 0,
        metadata_only_media_files_count: 0,
        metadata_only_other_files_count: 0,
        estimated_indexed_entries_count: 0,
        estimated_index_size_bytes: 0,
        truncated: false,
        risk_level: "warning",
        risk_reasons: [errorMessage],
        limitations: [errorMessage],
      });
    } finally {
      fileSearchRootEstimateLoading.value = false;
    }
  }

  async function confirmFileSearchRootAddition() {
    const path = fileSearchRootPendingPath.value;
    if (!path) return;
    fileSearchBusy.value = true;
    fileSearchError.value = "";
    const toastId = showFileSearchBusyToast("Добавляем папку поиска", shortenPath(path));
    try {
      await window.kepler.fileSearch.scopeAdd(path);
      closeFileSearchRootModal();
      await loadFileSearchState();
      watchFileSearchProgress("Папка добавлена, индексация запущена");
    } catch (err) {
      console.warn("file_index.scope_add failed", err);
      fileSearchRootEstimateError.value = describeFileSearchError(err, "Не удалось добавить папку");
      fileSearchError.value = describeFileSearchError(err, "Не удалось добавить папку поиска");
      finishFileSearchBusyToast(toastId, "Не удалось добавить папку", "info");
    } finally {
      fileSearchBusy.value = false;
    }
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
    await prepareFileSearchRootAddition(picked);
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
    fileSearchRootModalOpen,
    fileSearchRootEstimate,
    fileSearchRootEstimateLoading,
    fileSearchRootEstimateError,
    fileSearchRootPendingPath,
    fileSearchRootWarnings,
    fileSearchIndexTotalBytes,
    fileSearchIndexFilesCount,
    fileSearchIndexTotalBytesLabel,
    fileSearchIndexFilesCountLabel,
    fileSearchScanStateLabel,
    fileSearchProgressVisible,
    fileSearchProgressTitle,
    fileSearchProgressPercent,
    fileSearchProgressDetails,
    fileSearchProgressStallLabel,
    fileSearchRootConfirmLabel,
    fileSearchRootConfirmTone,
    fileSearchRootEstimateSummary,
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
    confirmFileSearchRootAddition,
    closeFileSearchRootModal,
  };
}
