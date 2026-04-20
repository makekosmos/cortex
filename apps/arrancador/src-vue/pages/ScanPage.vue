<script setup lang="ts">
import {
  Check,
  Cpu,
  FolderOpen,
  FolderSearch,
  Gamepad2,
  Loader2,
  Monitor,
  Plus,
  RefreshCw,
  Search,
  X,
} from "lucide-vue-next";
import { computed, onMounted, onUnmounted, reactive, shallowRef, watch } from "vue";
import { gamesApi, scanApi } from "../../src/lib/api";
import { pickDirectoryPath, subscribeAppEvent } from "../../src/lib/browser";
import { invoke } from "../../src/lib/ipc";
import type { ExeEntry, Game, NewGame } from "../../src/types";
import RawgMetadataPrompt from "../components/RawgMetadataPrompt.vue";
import { useToast } from "../composables/useToast";
import { useDropZone } from "../composables/useDropZone";
import { useGamesStore } from "../stores/games";

interface ScanResult extends ExeEntry {
  selected: boolean;
  alreadyAdded: boolean;
  customName: string;
  cpuUsage?: number;
  gpuUsage?: number;
}

function isSupportedDropPath(path: string) {
  const lower = path.toLowerCase();
  return lower.endsWith(".exe") || lower.endsWith(".lnk");
}

function fileNameFromPath(filePath: string) {
  const normalized = filePath.replace(/\\/g, "/");
  const name = normalized.split("/").pop();
  return name || filePath;
}

function cleanNameFromFile(fileName: string) {
  const base = fileName.replace(/\.exe$/i, "");
  return base.replace(/[-_]/g, " ").replace(/\s+/g, " ").trim() || base;
}

function normalizeNameForMerge(value: string) {
  return value
    .normalize("NFKD")
    .toLowerCase()
    .replace(/\.exe$/i, "")
    .replace(/\[[^\]]*\]|\([^\)]*\)|\{[^\}]*\}/g, " ")
    .replace(/[-_./]/g, " ")
    .replace(/[^\p{L}\p{N}\s]/gu, " ")
    .replace(/\s+/g, " ")
    .trim();
}

function matchMergeNames(candidate: string, incoming: string) {
  if (!candidate || !incoming) return false;
  if (candidate === incoming) return true;

  if (
    candidate.length > 4 &&
    incoming.length > 4 &&
    (candidate.includes(incoming) || incoming.includes(candidate))
  ) {
    return true;
  }

  const candidateTokens = candidate.split(" ").filter(Boolean);
  const incomingTokens = incoming.split(" ").filter(Boolean);
  if (candidateTokens.length <= 1 || incomingTokens.length <= 1) return false;

  const tokenSet = new Set(incomingTokens);
  const overlap = candidateTokens.filter((token) => tokenSet.has(token)).length;
  if (overlap < 2) return false;

  return overlap / Math.max(candidateTokens.length, incomingTokens.length) >= 0.67;
}

function findMergeCandidate(games: Game[], name: string): Game | undefined {
  const normalized = normalizeNameForMerge(name);
  if (!normalized) return undefined;

  for (const game of games) {
    const candidateNames = [game.name, game.exe_name, fileNameFromPath(game.exe_path)];
    const exact = candidateNames.find(
      (candidateName) => normalizeNameForMerge(candidateName) === normalized,
    );
    if (exact) return game;
  }

  return games.find((game) =>
    [game.name, game.exe_name, fileNameFromPath(game.exe_path)].some(
      (candidateName) => {
        const normalizedCandidate = normalizeNameForMerge(candidateName);
        return matchMergeNames(normalizedCandidate, normalized);
      },
    ),
  );
}

const gamesStore = useGamesStore();
const { notify } = useToast();

const state = reactive({
  activeTab: "folders" as "folders" | "processes",
  scanning: false,
  results: [] as ScanResult[],
  processes: [] as ScanResult[],
  filter: "",
  adding: false,
  sortBy: "name" as "name" | "cpu",
  loadingProcesses: false,
  error: null as string | null,
});

const metadataQueue = shallowRef<Game[]>([]);
let scanSession = 0;
let unlistenEntry: (() => void) | null = null;
let unlistenDone: (() => void) | null = null;

function enqueueMetadata(added: Game[]) {
  metadataQueue.value = [
    ...metadataQueue.value,
    ...added.filter(
      (candidate) => !metadataQueue.value.some((queued) => queued.id === candidate.id),
    ),
  ];
}

async function refreshUsage() {
  state.loadingProcesses = true;

  try {
    const processes = await scanApi.getRunningProcesses();
    if (state.processes.length === 0) {
      state.processes = processes.map((processEntry) => ({
        path: processEntry.path,
        file_name: processEntry.name,
        selected: false,
        alreadyAdded: false,
        customName: processEntry.name.replace(/\.exe$/i, ""),
        cpuUsage: processEntry.cpu_usage,
        gpuUsage: 0,
      }));
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

        return {
          path: processEntry.path,
          file_name: processEntry.name,
          selected: false,
          alreadyAdded: exists,
          customName: processEntry.name.replace(/\.exe$/i, ""),
          cpuUsage: processEntry.cpu_usage,
          gpuUsage: processEntry.gpu_usage,
        };
      }),
    );
  } catch (cause) {
    console.error("Failed to load processes:", cause);
    state.error = cause instanceof Error ? cause.message : String(cause);
  } finally {
    state.loadingProcesses = false;
  }
}

async function handleDroppedPaths(paths: string[]) {
  if (paths.length === 0) return;

  const toAdd: NewGame[] = [];
  const seen = new Set<string>();
  let skipped = 0;
  let invalid = 0;
  let merged = 0;

  for (const rawPath of paths) {
    let resolved = rawPath;
    if (rawPath.toLowerCase().endsWith(".lnk")) {
      try {
        resolved = await gamesApi.resolveShortcutTarget(rawPath);
      } catch (cause) {
        console.error("Failed to resolve shortcut:", cause);
        invalid += 1;
        continue;
      }
    }

    const lowerResolved = resolved.toLowerCase();
    if (!lowerResolved.endsWith(".exe")) {
      invalid += 1;
      continue;
    }

    if (seen.has(lowerResolved)) {
      continue;
    }

    seen.add(lowerResolved);
    const exists = await gamesApi.existsByPath(resolved).catch(() => false);
    if (exists) {
      skipped += 1;
      continue;
    }

    const exeName = fileNameFromPath(resolved);
    const name = cleanNameFromFile(exeName);
    const candidate = findMergeCandidate(gamesStore.games, name);
    if (candidate) {
      const doMerge = window.confirm(
        `Найдена игра с похожим названием: "${candidate.name}" из библиотеки. Обновить путь для существующей записи?`,
      );

      if (doMerge) {
        await gamesStore.updateGame(candidate.id, { exe_path: resolved });
        merged += 1;
        continue;
      }
    }

    toAdd.push({
      name,
      exe_path: resolved,
      exe_name: exeName,
    });
  }

  if (toAdd.length > 0) {
    try {
      const added = await gamesStore.addGames(toAdd);
      enqueueMetadata(added);

      const extra: string[] = [];
      if (skipped > 0) extra.push(`Пропущено: ${skipped}`);
      if (invalid > 0) extra.push(`Не поддерживается: ${invalid}`);
      if (merged > 0) extra.push(`Объединено с существующими: ${merged}`);

      notify({
        tone: "success",
        title: `Добавлено ${added.length}`,
        description: extra.length ? extra.join(" | ") : undefined,
      });
    } catch (cause) {
      console.error("Failed to add dropped games:", cause);
      notify({
        tone: "error",
        title: "Не удалось добавить игру",
        description: "Проверьте путь к файлу.",
      });
    }
    return;
  }

  if (merged > 0) {
    notify({
      tone: "success",
      title: `Объединено с существующими: ${merged}`,
      description: "Обновлены пути для игр с совпавшим названием.",
    });
    return;
  }

  notify({
    tone: "warning",
    title: "Файлы не добавлены",
    description: "Поддерживаются .exe и .lnk.",
  });
}

const { dropActive, dropHandlers } = useDropZone({
  onDropPaths: handleDroppedPaths,
});

watch(
  () => state.activeTab,
  (tab) => {
    state.sortBy = tab === "processes" ? "cpu" : "name";
  },
);

function handleTabChange(tab: "folders" | "processes") {
  state.activeTab = tab;
  if (tab === "processes" && state.processes.length === 0) {
    void loadProcesses();
  }
}

function toggleSelect(listType: "folders" | "processes", path: string) {
  const list = listType === "folders" ? state.results : state.processes;
  const next = list.map((entry) =>
    entry.path === path && !entry.alreadyAdded
      ? { ...entry, selected: !entry.selected }
      : entry,
  );

  if (listType === "folders") {
    state.results = next;
  } else {
    state.processes = next;
  }
}

function selectAll(listType: "folders" | "processes") {
  const list = listType === "folders" ? state.results : state.processes;
  const next = list.map((entry) =>
    !entry.alreadyAdded ? { ...entry, selected: true } : entry,
  );

  if (listType === "folders") {
    state.results = next;
  } else {
    state.processes = next;
  }
}

function deselectAll(listType: "folders" | "processes") {
  const list = listType === "folders" ? state.results : state.processes;
  const next = list.map((entry) => ({ ...entry, selected: false }));

  if (listType === "folders") {
    state.results = next;
  } else {
    state.processes = next;
  }
}

function updateName(listType: "folders" | "processes", path: string, name: string) {
  const list = listType === "folders" ? state.results : state.processes;
  const next = list.map((entry) =>
    entry.path === path ? { ...entry, customName: name } : entry,
  );

  if (listType === "folders") {
    state.results = next;
  } else {
    state.processes = next;
  }
}

async function addSelectedGames(listType: "folders" | "processes") {
  const list = listType === "folders" ? state.results : state.processes;
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
    enqueueMetadata(added);
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

    const nextList = list.map((entry) =>
      addedPaths.has(entry.path.toLowerCase())
        ? { ...entry, alreadyAdded: true, selected: false }
        : entry,
    );

    if (listType === "folders") {
      state.results = nextList;
    } else {
      state.processes = nextList;
    }
  } catch (cause) {
    console.error("Failed to add games:", cause);
  } finally {
    state.adding = false;
  }
}

const currentList = computed(() =>
  state.activeTab === "folders" ? state.results : state.processes,
);

const sortedList = computed(() =>
  [...currentList.value].sort((left, right) => {
    if (state.sortBy === "cpu") {
      return (right.cpuUsage ?? 0) - (left.cpuUsage ?? 0);
    }

    return left.customName.localeCompare(right.customName);
  }),
);

const filteredResults = computed(() => {
  const filterNeedle = state.filter.trim().toLowerCase();
  if (!filterNeedle) {
    return sortedList.value;
  }

  return sortedList.value.filter(
    (entry) =>
      entry.file_name.toLowerCase().includes(filterNeedle) ||
      entry.customName.toLowerCase().includes(filterNeedle),
  );
});

const selectedCount = computed(
  () => currentList.value.filter((entry) => entry.selected && !entry.alreadyAdded).length,
);
const newCount = computed(
  () => currentList.value.filter((entry) => !entry.alreadyAdded).length,
);
const currentMetadataGame = computed(() => metadataQueue.value[0] ?? null);

onMounted(() => {
  unlistenEntry = subscribeAppEvent<ExeEntry>("scan:entry", (payload) => {
    const sessionId = scanSession;
    void (async () => {
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

      if (state.results.some((entry) => entry.path.toLowerCase() === lowerResolved)) {
        return;
      }

      state.results = [
        ...state.results,
        {
          path: resolved,
          file_name: fileName,
          selected: !exists,
          alreadyAdded: exists,
          customName,
          cpuUsage: 0,
          gpuUsage: 0,
        },
      ];
    })();
  });

  unlistenDone = subscribeAppEvent<{ count: number }>("scan:done", () => {
    state.scanning = false;
  });
});

onUnmounted(() => {
  unlistenEntry?.();
  unlistenDone?.();
});
</script>

<template>
  <div
    class="flex h-full w-full max-w-5xl flex-col p-3 sm:p-6"
    @dragenter="dropHandlers.onDragenter"
    @dragover="dropHandlers.onDragover"
    @dragleave="dropHandlers.onDragleave"
    @drop="dropHandlers.onDrop"
  >
    <div class="mb-4 sm:mb-6">
      <h1 class="text-xl font-bold tracking-tight sm:text-2xl">Добавление игр</h1>
      <p class="text-xs text-muted-foreground sm:text-sm">
        Найдите игры на диске или выберите из запущенных процессов
      </p>
    </div>

    <div class="mb-4 flex w-full space-x-1 rounded-lg bg-muted p-1 sm:mb-6 sm:w-fit">
      <button
        type="button"
        class="flex flex-1 items-center justify-center gap-2 rounded-md px-3 py-2 text-xs font-medium transition-all sm:flex-none sm:px-4 sm:text-sm"
        :class="
          state.activeTab === 'folders'
            ? 'bg-background text-foreground shadow-sm'
            : 'text-muted-foreground hover:bg-background/50'
        "
        @click="handleTabChange('folders')"
      >
        <FolderSearch class="h-4 w-4" />
        <span class="truncate">Папки</span>
      </button>
      <button
        type="button"
        class="flex flex-1 items-center justify-center gap-2 rounded-md px-3 py-2 text-xs font-medium transition-all sm:flex-none sm:px-4 sm:text-sm"
        :class="
          state.activeTab === 'processes'
            ? 'bg-background text-foreground shadow-sm'
            : 'text-muted-foreground hover:bg-background/50'
        "
        @click="handleTabChange('processes')"
      >
        <Cpu class="h-4 w-4" />
        <span class="truncate">Процессы</span>
      </button>
    </div>

    <div class="mb-4 flex flex-col items-stretch gap-3 sm:mb-6 sm:flex-row sm:items-center sm:gap-4">
      <template v-if="state.activeTab === 'folders'">
        <template v-if="state.scanning">
          <button
            type="button"
            class="inline-flex items-center justify-center gap-2 rounded-xl bg-destructive px-4 py-2 text-sm text-white"
            @click="void cancelScan()"
          >
            <X class="h-4 w-4" />
            Остановить
          </button>

          <div class="flex flex-1 flex-col gap-2">
            <div class="flex justify-between text-[10px] sm:text-sm">
              <span class="flex items-center gap-2 font-medium text-primary">
                <Loader2 class="h-3 w-3 animate-spin" />
                Сканирование...
              </span>
              <span class="text-muted-foreground">Найдено: {{ state.results.length }}</span>
            </div>
            <div class="h-2 w-full overflow-hidden rounded-full bg-muted">
              <div class="h-full w-1/3 animate-pulse rounded-full bg-primary" />
            </div>
          </div>
        </template>

        <button
          v-else
          type="button"
          class="inline-flex w-full items-center justify-center gap-2 rounded-xl bg-primary px-4 py-2 text-sm font-[510] text-primary-foreground transition-colors hover:bg-accent hover:text-white sm:w-auto"
          @click="void startScan()"
        >
          <FolderOpen class="h-4 w-4" />
          Выбрать папку
        </button>
      </template>

      <div v-else class="flex w-full items-center gap-2 sm:w-auto">
        <button
          type="button"
          class="inline-flex flex-1 items-center justify-center gap-2 rounded-xl bg-primary px-4 py-2 text-sm font-[510] text-primary-foreground transition-colors hover:bg-accent hover:text-white sm:flex-none"
          :disabled="state.loadingProcesses"
          @click="void loadProcesses()"
        >
          <Loader2 v-if="state.loadingProcesses" class="h-4 w-4 animate-spin" />
          <RefreshCw v-else class="h-4 w-4" />
          Обновить список
        </button>
      </div>
    </div>

    <div v-if="currentList.length > 0 || state.loadingProcesses" class="mb-4 flex flex-col gap-3">
      <div class="relative w-full">
        <Search class="absolute top-1/2 left-3 h-4 w-4 -translate-y-1/2 text-muted-foreground" />
        <input
          v-model="state.filter"
          placeholder="Поиск по названию или файлу..."
          class="h-10 w-full rounded-xl border border-border/70 bg-card/70 pl-9 pr-9 text-sm outline-none"
        />
        <button
          v-if="state.filter"
          type="button"
          class="absolute top-1/2 right-3 -translate-y-1/2 text-muted-foreground hover:text-foreground"
          @click="state.filter = ''"
        >
          <X class="h-4 w-4" />
        </button>
      </div>

      <div class="flex items-center justify-between gap-2 overflow-x-auto pb-1">
        <div class="flex flex-shrink-0 items-center gap-2">
          <div v-if="state.activeTab === 'processes'" class="flex items-center rounded-md bg-muted p-0.5">
            <button
              type="button"
              class="rounded-sm px-2 py-1 text-[10px] font-medium transition-all sm:text-xs"
              :class="
                state.sortBy === 'cpu'
                  ? 'bg-background text-foreground shadow-sm'
                  : 'text-muted-foreground hover:text-foreground'
              "
              @click="state.sortBy = 'cpu'"
            >
              ЦП
            </button>
            <div class="mx-1 h-3 w-px bg-border" />
            <button
              type="button"
              class="px-1.5 py-1 text-muted-foreground transition-all hover:text-foreground disabled:opacity-50"
              :disabled="state.loadingProcesses"
              title="Обновить нагрузку"
              @click="void refreshUsage()"
            >
              <Loader2 class="h-3 w-3" :class="{ 'animate-spin': state.loadingProcesses }" />
            </button>
          </div>

          <span class="whitespace-nowrap text-[10px] text-muted-foreground sm:text-sm">
            {{ selectedCount }} из {{ newCount }}
          </span>
        </div>

        <div class="flex flex-shrink-0 items-center gap-1.5">
          <button
            type="button"
            class="h-7 rounded-lg border border-border/70 px-2 text-[10px] transition-colors hover:bg-accent/70 sm:h-8 sm:px-3 sm:text-xs"
            @click="selectAll(state.activeTab)"
          >
            Все
          </button>
          <button
            type="button"
            class="h-7 rounded-lg border border-border/70 px-2 text-[10px] transition-colors hover:bg-accent/70 sm:h-8 sm:px-3 sm:text-xs"
            @click="deselectAll(state.activeTab)"
          >
            Сброс
          </button>
        </div>
      </div>
    </div>

    <div class="flex min-h-0 flex-1 flex-col">
      <div
        v-if="state.error"
        class="flex flex-1 flex-col items-center justify-center rounded-lg border border-destructive/20 bg-destructive/5 p-6 text-center"
      >
        <div class="mb-4 flex h-12 w-12 items-center justify-center rounded-full bg-destructive/10 text-destructive">
          <X class="h-6 w-6" />
        </div>
        <h3 class="mb-2 text-lg font-semibold text-destructive">Произошла ошибка</h3>
        <p class="max-w-md break-all rounded border bg-background/50 p-3 font-mono text-sm text-muted-foreground">
          {{ state.error }}
        </p>
        <button
          type="button"
          class="mt-4 inline-flex items-center gap-2 rounded-xl border border-border/70 px-4 py-2 text-sm transition-colors hover:bg-accent/70"
          @click="void loadProcesses()"
        >
          <Monitor class="h-4 w-4" />
          Попробовать снова
        </button>
      </div>

      <div
        v-else-if="state.loadingProcesses"
        class="flex flex-1 flex-col items-center justify-center rounded-lg border p-4"
      >
        <Loader2 class="mb-2 h-8 w-8 animate-spin text-primary" />
        <p class="text-xs text-muted-foreground sm:text-sm">Получение списка процессов...</p>
      </div>

      <div v-else-if="filteredResults.length > 0" class="flex-1 overflow-auto rounded-lg border">
        <div class="space-y-1 p-1.5 sm:p-2">
          <div
            v-for="result in filteredResults"
            :key="result.path"
            class="flex items-center gap-3 rounded-lg border border-transparent p-2 transition-all duration-300 sm:p-3"
            :class="
              result.alreadyAdded
                ? 'bg-muted/20 opacity-60'
                : result.selected
                  ? 'border-primary/20 bg-primary/10'
                  : 'bg-card shadow-sm hover:bg-accent'
            "
          >
            <button
              type="button"
              class="flex h-5 w-5 flex-shrink-0 items-center justify-center rounded border-2 transition-colors"
              :class="
                result.alreadyAdded
                  ? 'cursor-not-allowed border-muted bg-muted'
                  : result.selected
                    ? 'border-primary bg-primary text-primary-foreground'
                    : 'border-border hover:border-primary'
              "
              :disabled="result.alreadyAdded"
              @click="toggleSelect(state.activeTab, result.path)"
            >
              <Check v-if="result.selected || result.alreadyAdded" class="h-3 w-3" />
            </button>

            <div class="relative flex h-10 w-10 flex-shrink-0 items-center justify-center overflow-hidden rounded bg-secondary shadow-sm">
              <Gamepad2 class="h-5 w-5 text-muted-foreground" />
            </div>

            <div class="flex min-w-0 flex-1 flex-col overflow-hidden">
              <div class="flex items-center gap-2">
                <span
                  v-if="result.alreadyAdded"
                  class="truncate text-sm font-semibold leading-tight sm:text-base"
                >
                  {{ result.customName }}
                </span>
                <input
                  v-else
                  :value="result.customName"
                  placeholder="Название"
                  class="-ml-1 w-fit max-w-full rounded bg-transparent px-1 text-sm font-semibold transition-colors hover:bg-accent/50 focus:outline-none sm:text-base"
                  :style="{ width: `${Math.max(result.customName.length, 1)}ch` }"
                  @input="
                    updateName(
                      state.activeTab,
                      result.path,
                      ($event.target as HTMLInputElement).value,
                    )
                  "
                />

                <span
                  v-if="result.alreadyAdded"
                  class="flex-shrink-0 rounded bg-muted px-1.5 py-0.5 text-[9px] font-bold uppercase leading-none tracking-wider text-muted-foreground"
                >
                  Есть
                </span>
              </div>
              <div class="mt-0.5 truncate pr-4 text-[10px] text-muted-foreground opacity-50 sm:text-xs">
                {{ result.path }}
              </div>
            </div>

            <div class="ml-auto flex flex-shrink-0 flex-col items-end gap-1 pl-2">
              <div
                class="flex items-center gap-1.5 rounded-md border px-2 py-1 transition-colors duration-500"
                :class="
                  (result.cpuUsage ?? 0) > 1
                    ? 'border-primary/20 bg-primary/10 text-primary'
                    : 'border-primary/10 bg-primary/5 text-primary/80'
                "
              >
                <span class="text-[9px] font-bold uppercase tracking-tighter opacity-70">
                  CPU
                </span>
                <span class="min-w-[35px] text-right font-mono text-[10px] font-bold sm:text-xs">
                  {{ (result.cpuUsage ?? 0).toFixed(1) }}%
                </span>
              </div>
            </div>
          </div>
        </div>
      </div>

      <div
        v-else
        class="flex flex-1 flex-col items-center justify-center rounded-lg border p-6 text-center sm:p-8"
      >
        <FolderSearch
          v-if="state.activeTab === 'folders'"
          class="mb-4 h-10 w-10 text-muted-foreground/30 sm:h-12 sm:w-12"
        />
        <Cpu
          v-else
          class="mb-4 h-10 w-10 text-muted-foreground/30 sm:h-12 sm:w-12"
        />
        <h3 class="text-base font-medium sm:text-lg">
          {{ state.filter ? "Ничего не найдено" : "Список пуст" }}
        </h3>
        <p class="mt-2 max-w-xs text-xs text-muted-foreground sm:text-sm">
          {{
            state.filter
              ? `По запросу "${state.filter}" ничего не найдено.`
              : state.activeTab === "folders"
                ? "Выберите папку для поиска игр."
                : "Нажмите 'Обновить список' для поиска процессов."
          }}
        </p>
      </div>
    </div>

    <div
      v-if="selectedCount > 0"
      class="sticky bottom-0 mt-4 flex flex-col items-center justify-between gap-3 border-t bg-background/80 pt-4 backdrop-blur-sm sm:mt-6 sm:flex-row sm:pt-6"
    >
      <div class="text-xs sm:text-sm">
        Выбрано <span class="font-bold text-primary">{{ selectedCount }}</span>
        {{ selectedCount === 1 ? "игра" : "игр" }}
      </div>
      <button
        type="button"
        class="inline-flex w-full items-center justify-center gap-2 rounded-xl bg-primary px-8 py-2 text-sm font-[510] text-primary-foreground transition-colors hover:bg-accent hover:text-white sm:w-auto"
        :disabled="state.adding"
        @click="void addSelectedGames(state.activeTab)"
      >
        <Loader2 v-if="state.adding" class="h-4 w-4 animate-spin" />
        <Plus v-else class="h-4 w-4" />
        Добавить
      </button>
    </div>

    <RawgMetadataPrompt
      v-if="currentMetadataGame"
      :game="currentMetadataGame"
      :remaining="metadataQueue.length"
      @next="metadataQueue = metadataQueue.slice(1)"
      @skip-all="metadataQueue = []"
      @after-apply="void gamesStore.refreshGames()"
    />

    <div v-if="dropActive" class="pointer-events-none fixed inset-0 z-50">
      <div class="absolute inset-0 bg-background/40 backdrop-blur-sm" />
      <div class="absolute inset-3 rounded-2xl border-2 border-dashed border-primary/70 shadow-[0_0_0_1px_rgba(255,255,255,0.06),0_30px_80px_rgba(8,12,24,0.55)] sm:inset-6">
        <div class="absolute inset-0 flex items-center justify-center">
          <div class="rounded-2xl border border-border/60 bg-card/80 px-6 py-4 text-center shadow-[0_18px_45px_rgba(8,12,24,0.45)] backdrop-blur-xl">
            <div class="text-sm font-semibold text-foreground">Отпустите, чтобы добавить игру</div>
            <div class="text-xs text-muted-foreground">Поддерживаются .exe и .lnk</div>
          </div>
        </div>
      </div>
    </div>
  </div>
</template>
