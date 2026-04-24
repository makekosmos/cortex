import { describe, expect, it } from "vitest";
import {
  deselectAllScanResults,
  filteredScanResults,
  hasScanResultPath,
  markScanResultsAdded,
  renameScanResult,
  type ScanResult,
  scanResultFromExecutable,
  scanResultFromProcess,
  selectAllScanResults,
  sortedScanResults,
  toggleScanResultSelection,
} from "../lib/scanResults";

const makeResult = (overrides: Partial<ScanResult>): ScanResult => ({
  path: "C:\\Games\\Control\\Control.exe",
  file_name: "Control.exe",
  selected: false,
  alreadyAdded: false,
  customName: "Control",
  cpuUsage: 0,
  gpuUsage: 0,
  ...overrides,
});

describe("scan result helpers", () => {
  it("creates executable and process scan entries with stable defaults", () => {
    expect(
      scanResultFromExecutable(
        "C:\\Games\\Control\\Control.exe",
        "Control.exe",
        "Control",
        false,
      ),
    ).toMatchObject({
      selected: true,
      alreadyAdded: false,
      cpuUsage: 0,
      gpuUsage: 0,
    });

    expect(
      scanResultFromProcess(
        {
          pid: 42,
          name: "Hades.exe",
          path: "D:\\Hades\\Hades.exe",
          cpu_usage: 12,
          gpu_usage: 8,
        },
        true,
      ),
    ).toMatchObject({
      customName: "Hades",
      selected: false,
      alreadyAdded: true,
      cpuUsage: 12,
      gpuUsage: 8,
    });
  });

  it("keeps selection immutable and blocks already-added entries", () => {
    const results = [
      makeResult({ path: "a.exe" }),
      makeResult({ path: "b.exe", alreadyAdded: true }),
    ];

    expect(toggleScanResultSelection(results, "a.exe")[0]?.selected).toBe(true);
    expect(toggleScanResultSelection(results, "b.exe")[1]?.selected).toBe(false);
    expect(selectAllScanResults(results).map((entry) => entry.selected)).toEqual([
      true,
      false,
    ]);
    expect(
      deselectAllScanResults([
        makeResult({ path: "a.exe", selected: true }),
        makeResult({ path: "b.exe", selected: true }),
      ]).map((entry) => entry.selected),
    ).toEqual([false, false]);
  });

  it("renames, marks added paths, filters, and sorts results", () => {
    const results = [
      makeResult({ path: "b.exe", customName: "Beta", cpuUsage: 3 }),
      makeResult({ path: "a.exe", customName: "Alpha", cpuUsage: 9 }),
    ];

    expect(hasScanResultPath(results, "A.EXE")).toBe(true);
    expect(renameScanResult(results, "b.exe", "Beta Prime")[0]?.customName).toBe(
      "Beta Prime",
    );
    expect(
      markScanResultsAdded(results, new Set(["a.exe"])).map((entry) => ({
        path: entry.path,
        selected: entry.selected,
        alreadyAdded: entry.alreadyAdded,
      })),
    ).toEqual([
      { path: "b.exe", selected: false, alreadyAdded: false },
      { path: "a.exe", selected: false, alreadyAdded: true },
    ]);
    expect(sortedScanResults(results, "name").map((entry) => entry.customName)).toEqual([
      "Alpha",
      "Beta",
    ]);
    expect(sortedScanResults(results, "cpu").map((entry) => entry.path)).toEqual([
      "a.exe",
      "b.exe",
    ]);
    expect(filteredScanResults(results, "alp").map((entry) => entry.path)).toEqual([
      "a.exe",
    ]);
  });
});
