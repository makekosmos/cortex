<script setup lang="ts">
import {
  Cpu,
  FolderOpen,
  FolderSearch,
  Loader2,
  RefreshCw,
  X,
} from "lucide-vue-next";
import RawgMetadataPrompt from "../components/RawgMetadataPrompt.vue";
import ScanResultsList from "../components/scan/ScanResultsList.vue";
import { useScanPageState } from "../composables/useScanPageState";

const {
  state,
  currentList,
  filteredResults,
  selectedCount,
  newCount,
  metadataQueue,
  currentMetadataGame,
  dropActive,
  dropHandlers,
  refreshGames,
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
  advanceMetadataQueue,
  clearMetadataQueue,
} = useScanPageState();
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
          data-testid="scan-start"
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

    <ScanResultsList
      v-model:filter="state.filter"
      v-model:sort-by="state.sortBy"
      :active-tab="state.activeTab"
      :current-list-length="currentList.length"
      :filtered-results="filteredResults"
      :selected-count="selectedCount"
      :new-count="newCount"
      :loading-processes="state.loadingProcesses"
      :adding="state.adding"
      :error="state.error"
      @refresh-usage="void refreshUsage()"
      @load-processes="void loadProcesses()"
      @select-all="selectAll"
      @deselect-all="deselectAll"
      @toggle-select="toggleSelect"
      @update-name="updateName"
      @add-selected="void addSelectedGames($event)"
    />

    <RawgMetadataPrompt
      v-if="currentMetadataGame"
      :game="currentMetadataGame"
      :remaining="metadataQueue.length"
      @next="advanceMetadataQueue"
      @skip-all="clearMetadataQueue"
      @after-apply="void refreshGames()"
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
