import type {
  FileIndexSettings,
  FileSearchDiagnosticsReport,
  FileSearchRootWarning,
} from "@shared/ipc-types";

export interface FileSearchPatch {
  exclude_noisy_folders?: boolean;
  respect_gitignore?: boolean;
  include_hidden?: boolean;
  ntfs_accelerated?: boolean;
}

const FILE_SEARCH_ERROR_MARKERS = [
  "pattern уже есть",
  "invalid ignore pattern",
  "must not be empty",
  "must be an existing directory",
];

export function formatCount(value: number): string {
  return new Intl.NumberFormat("ru-RU").format(value);
}

export function shortenPath(path: string): string {
  if (path.length <= 42) return path;
  return `${path.slice(0, 18)}…${path.slice(-20)}`;
}

export function describeFileSearchError<T>(err: T, fallback: string): string {
// SAFETY: the surrounding domain validation preserves the asserted contract.
  const raw = (err as { message?: string } | null)?.message ?? String(err ?? "");
  const trimmed = raw.trim();
  if (!trimmed) return fallback;
  for (const marker of FILE_SEARCH_ERROR_MARKERS) {
    if (trimmed.toLowerCase().includes(marker.toLowerCase())) {
      return trimmed.replace(/^[a-z_]+\.[a-z_]+:\s*/i, "");
    }
  }
  return `${fallback}: ${trimmed}`;
}

export function formatFileSearchProgress(settings: FileIndexSettings | null): string {
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

export function buildFileSearchRootWarnings(
  diagnostics: FileSearchDiagnosticsReport | null,
): FileSearchRootWarning[] {
  if (!diagnostics || diagnostics.risk_level === "ok" || diagnostics.risk_reasons.length === 0) {
    return [];
  }
  return diagnostics.roots.map((root) => ({
    path: root,
    risk_level: diagnostics.risk_level === "danger" ? "danger" : "warning",
    risk_reasons: diagnostics.risk_reasons,
  }));
}
