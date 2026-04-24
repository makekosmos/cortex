import type { ExeEntry, ProcessEntry } from "@/types";

export type ScanListType = "folders" | "processes";
export type ScanSortBy = "name" | "cpu";

export interface ScanResult extends ExeEntry {
  selected: boolean;
  alreadyAdded: boolean;
  customName: string;
  cpuUsage?: number;
  gpuUsage?: number;
}

export function scanResultFromProcess(
  processEntry: ProcessEntry,
  alreadyAdded: boolean,
): ScanResult {
  return {
    path: processEntry.path,
    file_name: processEntry.name,
    selected: false,
    alreadyAdded,
    customName: processEntry.name.replace(/\.exe$/i, ""),
    cpuUsage: processEntry.cpu_usage,
    gpuUsage: processEntry.gpu_usage,
  };
}

export function scanResultFromExecutable(
  path: string,
  fileName: string,
  customName: string,
  alreadyAdded: boolean,
): ScanResult {
  return {
    path,
    file_name: fileName,
    selected: !alreadyAdded,
    alreadyAdded,
    customName,
    cpuUsage: 0,
    gpuUsage: 0,
  };
}

export function hasScanResultPath(results: readonly ScanResult[], path: string) {
  const normalizedPath = path.toLowerCase();
  return results.some((entry) => entry.path.toLowerCase() === normalizedPath);
}

export function toggleScanResultSelection(
  results: readonly ScanResult[],
  path: string,
) {
  return results.map((entry) =>
    entry.path === path && !entry.alreadyAdded
      ? { ...entry, selected: !entry.selected }
      : entry,
  );
}

export function selectAllScanResults(results: readonly ScanResult[]) {
  return results.map((entry) =>
    !entry.alreadyAdded ? { ...entry, selected: true } : entry,
  );
}

export function deselectAllScanResults(results: readonly ScanResult[]) {
  return results.map((entry) => ({ ...entry, selected: false }));
}

export function renameScanResult(
  results: readonly ScanResult[],
  path: string,
  customName: string,
) {
  return results.map((entry) =>
    entry.path === path ? { ...entry, customName } : entry,
  );
}

export function markScanResultsAdded(
  results: readonly ScanResult[],
  paths: ReadonlySet<string>,
) {
  return results.map((entry) =>
    paths.has(entry.path.toLowerCase())
      ? { ...entry, alreadyAdded: true, selected: false }
      : entry,
  );
}

export function sortedScanResults(
  results: readonly ScanResult[],
  sortBy: ScanSortBy,
) {
  return [...results].sort((left, right) => {
    if (sortBy === "cpu") {
      return (right.cpuUsage ?? 0) - (left.cpuUsage ?? 0);
    }

    return left.customName.localeCompare(right.customName);
  });
}

export function filteredScanResults(
  results: readonly ScanResult[],
  filter: string,
) {
  const filterNeedle = filter.trim().toLowerCase();
  if (!filterNeedle) {
    return results;
  }

  return results.filter(
    (entry) =>
      entry.file_name.toLowerCase().includes(filterNeedle) ||
      entry.customName.toLowerCase().includes(filterNeedle),
  );
}
