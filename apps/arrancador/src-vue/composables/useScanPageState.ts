import { computed, onMounted, onUnmounted, reactive, watch } from "vue";
import { gamesApi, scanApi } from "../../src/lib/api";
import { pickDirectoryPath, subscribeAppEvent } from "../../src/lib/browser";
import { invoke } from "../../src/lib/ipc";
import type { ExeEntry, NewGame } from "../../src/types";
import {
  cleanNameFromFile,
  fileNameFromPath,
  findMergeCandidate,
  isSupportedDropPath,
} from "../lib/gameImport";
import {
  deselectAllScanResults,
  filteredScanResults,
  hasScanResultPath,
  markScanResultsAdded,
  renameScanResult,
  type ScanListType,
  type ScanResult,
  type ScanSortBy,
  scanResultFromExecutable,
  scanResultFromProcess,
  selectAllScanResults,
  sortedScanResults,
  toggleScanResultSelection,
} from "../lib/scanResults";
import { useGamesStore } from "../stores/games";
import { useLibraryGameImport } from "./useLibraryGameImport";
import { useToast } from "./useToast";

type ScanPageTab = ScanListType;

export function useScanPageState() {
  const gamesStore = useGamesStore();
  const { notify } = useToast();

  const state = reactive({
    activeTab: "folders" as ScanPageTab,
    scanning: false,
    results: [] as ScanResult[],
    processes: [] as ScanResult[],
    filter: "",
    adding: false,
    sortBy: "name" as ScanSortBy,
    loadingProcesses: false,
    error: null as string | null,
  });

  const gameImport = useLibraryGameImport({
    games: () => gamesStore.games,
    addGames: (games) => gamesStore.addGames(games),
    updateGame: (id, updates) => gamesStore.updateGame(id, updates),
    notify,
  });

  let scanSession = 0;
  let unlistenEntry: (() => void) | null = null;
  let unlistenDone: (() => void) | null = null;

  function listFor(listType: ScanListType) {
    return listType === "folders" ? state.results : state.processes;
  }

  function replaceList(listType: ScanListType, next: ScanResult[]) {
    if (listType === "folders") {
      state.results = next;
      return;
    }

    state.processes = next;
  }

  async function refreshUsage() {
    state.loadingProcesses = true;

    try {
      const processes = await scanApi.getRunningProcesses();
      if (state.processes.length === 0) {
        state.processes = processes.map((processEntry) =>
          scanResultFromProcess(processEntry, false),
        );
        return;
      }

      state.processes = state.processes.map((entry) => {
        const freshData = processes.find((candidate) => candidate.path === entry.path);
        return {
          ...entry,
          cpuUsage: freshData?.cpu_usage ?? 0,
        };
      });
    } catch (cause) {
      console.error("Failed to refresh usage:", cause);
    } finally {
      state.loadingProcesses = false;
    }
  }

  async function startScan() {
    if (state.scanning) return;

    const directory = await pickDirectoryPath({
      title: "Выбрать папку для сканирования",
    });

    if (!directory) {
      return;
    }

    scanSession += 1;
    state.results = [];
    state.scanning = true;

    try {
      await invoke("scan_executables_stream", { dir: directory });
    } catch (cause) {
      console.error("Scan failed:", cause);
      state.scanning = false;
    }
  }

  async function cancelScan() {
    scanSession += 1;
    state.scanning = false;

    try {
      await invoke("cancel_scan");
    } catch (cause) {
      console.error("Failed to cancel scan:", cause);
    }
  }

  async function loadProcesses() {
    state.loadingProcesses = true;
    state.processes = [];
    state.error = null;

    try {
      const processes = await scanApi.getRunningProcesses();

      if (processes.length === 0) {
        state.error =
          "nvidia-smi не вернул процессов. Проверьте, установлены ли драйвера NVIDIA и запущена ли игра.";
      }

      state.processes = await Promise.all(
        processes.map(async (processEntry) => {
          const exists = await gamesApi.existsByPath(processEntry.path).catch(() => false);

          return scanResultFromProcess(processEntry, exists);
        }),
      );
    } catch (cause) {
      console.error("Failed to load processes:", cause);
      state.error = cause instanceof Error ? cause.message : String(cause);
    } finally {
      state.loadingProcesses = false;
    }
  }

  function handleTabChange(tab: ScanPageTab) {
    state.activeTab = tab;
    if (tab === "processes" && state.processes.length === 0) {
      void loadProcesses();
    }
  }

  function toggleSelect(listType: ScanListType, path: string) {
    replaceList(listType, toggleScanResultSelection(listFor(listType), path));
  }

  function selectAll(listType: ScanListType) {
    replaceList(listType, selectAllScanResults(listFor(listType)));
  }

  function deselectAll(listType: ScanListType) {
    replaceList(listType, deselectAllScanResults(listFor(listType)));
  }

  function updateName(listType: ScanListType, path: string, name: string) {
    replaceList(listType, renameScanResult(listFor(listType), path, name));
  }

  async function addSelectedGames(listType: ScanListType) {
    const list = listFor(listType);
    const selected = list.filter((entry) => entry.selected && !entry.alreadyAdded);
    if (selected.length === 0) {
      return;
    }

    state.adding = true;

    try {
      const newGames: NewGame[] = [];
      const mergedPaths = new Set<string>();
      let merged = 0;

      for (const entry of selected) {
        const name = entry.customName || entry.file_name.replace(/\.exe$/i, "");
        const candidate = findMergeCandidate(gamesStore.games, name);

        if (candidate) {
          const doMerge = window.confirm(
            `Найдена игра с похожим названием: "${candidate.name}" из библиотеки. Обновить путь для существующей записи?`,
          );

          if (doMerge) {
            await gamesStore.updateGame(candidate.id, { exe_path: entry.path });
            merged += 1;
            mergedPaths.add(entry.path.toLowerCase());
            continue;
          }
        }

        newGames.push({
          name,
          exe_path: entry.path,
          exe_name: entry.file_name,
        });
      }

      const added = newGames.length > 0 ? await gamesStore.addGames(newGames) : [];
      gameImport.enqueueMetadata(added);
      await gamesStore.refreshGames();

      if (merged > 0) {
        notify({
          tone: "success",
          title: `Объединено с существующими: ${merged}`,
          description: `Обновлены пути для ${merged} игры(и) с совпадением названия.`,
        });
      }

      const addedPaths = new Set(
        added.map((game) => game.exe_path.toLowerCase()).concat([...mergedPaths]),
      );

      replaceList(listType, markScanResultsAdded(list, addedPaths));
    } catch (cause) {
      console.error("Failed to add games:", cause);
    } finally {
      state.adding = false;
    }
  }

  async function addScanEntry(payload: ExeEntry, sessionId: number) {
    const rawPath = payload?.path?.trim();
    if (!rawPath || !isSupportedDropPath(rawPath)) {
      return;
    }

    let resolved = rawPath;
    if (rawPath.toLowerCase().endsWith(".lnk")) {
      try {
        resolved = await gamesApi.resolveShortcutTarget(rawPath);
      } catch (cause) {
        console.error("Failed to resolve shortcut:", cause);
        return;
      }
    }

    const lowerResolved = resolved.toLowerCase();
    if (!lowerResolved.endsWith(".exe")) {
      return;
    }

    const exists = await gamesApi.existsByPath(resolved).catch(() => false);
    const fileName = fileNameFromPath(resolved);
    const customName = cleanNameFromFile(fileName);

    if (sessionId !== scanSession) {
      return;
    }

    if (hasScanResultPath(state.results, lowerResolved)) {
      return;
    }

    state.results = [
      ...state.results,
      scanResultFromExecutable(resolved, fileName, customName, exists),
    ];
  }

  const currentList = computed(() => listFor(state.activeTab));

  const sortedList = computed(() =>
    sortedScanResults(currentList.value, state.sortBy),
  );

  const filteredResults = computed(() =>
    filteredScanResults(sortedList.value, state.filter),
  );

  const selectedCount = computed(
    () =>
      currentList.value.filter((entry) => entry.selected && !entry.alreadyAdded).length,
  );

  const newCount = computed(
    () => currentList.value.filter((entry) => !entry.alreadyAdded).length,
  );

  watch(
    () => state.activeTab,
    (tab) => {
      state.sortBy = tab === "processes" ? "cpu" : "name";
    },
  );

  onMounted(() => {
    unlistenEntry = subscribeAppEvent<ExeEntry>("scan:entry", (payload) => {
      const sessionId = scanSession;
      void addScanEntry(payload, sessionId);
    });

    unlistenDone = subscribeAppEvent<{ count: number }>("scan:done", () => {
      state.scanning = false;
    });
  });

  onUnmounted(() => {
    unlistenEntry?.();
    unlistenDone?.();
  });

  return {
    state,
    currentList,
    sortedList,
    filteredResults,
    selectedCount,
    newCount,
    metadataQueue: gameImport.metadataQueue,
    currentMetadataGame: gameImport.currentMetadataGame,
    dropActive: gameImport.dropActive,
    dropHandlers: gameImport.dropHandlers,
    refreshGames: gamesStore.refreshGames,
    refreshUsage,
    startScan,
    cancelScan,
    loadProcesses,
    handleTabChange,
    toggleSelect,
    selectAll,
    deselectAll,
    updateName,
    addSelectedGames,
    advanceMetadataQueue: gameImport.advanceMetadataQueue,
    clearMetadataQueue: gameImport.clearMetadataQueue,
  };
}
