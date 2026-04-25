<script setup lang="ts">
import {
  Check,
  Cpu,
  FolderSearch,
  Gamepad2,
  Loader2,
  Monitor,
  Plus,
  Search,
  X,
} from "lucide-vue-next";

interface ScanResult {
  path: string;
  file_name: string;
  selected: boolean;
  alreadyAdded: boolean;
  customName: string;
  cpuUsage?: number;
  gpuUsage?: number;
}

interface Props {
  activeTab: "folders" | "processes";
  currentListLength: number;
  filteredResults: readonly ScanResult[];
  selectedCount: number;
  newCount: number;
  loadingProcesses: boolean;
  adding: boolean;
  error: string | null;
  filter: string;
  sortBy: "name" | "cpu";
}

interface Emits {
  "update:filter": [value: string];
  "update:sortBy": [value: "name" | "cpu"];
  refreshUsage: [];
  loadProcesses: [];
  selectAll: [listType: "folders" | "processes"];
  deselectAll: [listType: "folders" | "processes"];
  toggleSelect: [listType: "folders" | "processes", path: string];
  updateName: [listType: "folders" | "processes", path: string, name: string];
  addSelected: [listType: "folders" | "processes"];
}

const props = defineProps<Props>();
const emit = defineEmits<Emits>();
</script>

<template>
  <div v-if="props.currentListLength > 0 || props.loadingProcesses" class="mb-4 flex flex-col gap-3">
    <div class="relative w-full">
      <Search class="absolute top-1/2 left-3 h-4 w-4 -translate-y-1/2 text-muted-foreground" />
      <input
        :value="props.filter"
        placeholder="Поиск по названию или файлу..."
        class="h-10 w-full rounded-xl border border-border/70 bg-card/70 pl-9 pr-9 text-sm outline-none"
        @input="emit('update:filter', ($event.target as HTMLInputElement).value)"
      />
      <button
        v-if="props.filter"
        type="button"
        class="absolute top-1/2 right-3 -translate-y-1/2 text-muted-foreground hover:text-foreground"
        @click="emit('update:filter', '')"
      >
        <X class="h-4 w-4" />
      </button>
    </div>

    <div class="flex items-center justify-between gap-2 overflow-x-auto pb-1">
      <div class="flex flex-shrink-0 items-center gap-2">
        <div v-if="props.activeTab === 'processes'" class="flex items-center rounded-md bg-muted p-0.5">
          <button
            type="button"
            class="rounded-sm px-2 py-1 text-[10px] font-medium transition-all sm:text-xs"
            :class="
              props.sortBy === 'cpu'
                ? 'bg-background text-foreground shadow-sm'
                : 'text-muted-foreground hover:text-foreground'
            "
            @click="emit('update:sortBy', 'cpu')"
          >
            ЦП
          </button>
          <div class="mx-1 h-3 w-px bg-border" />
          <button
            type="button"
            class="px-1.5 py-1 text-muted-foreground transition-all hover:text-foreground disabled:opacity-50"
            :disabled="props.loadingProcesses"
            title="Обновить нагрузку"
            @click="emit('refreshUsage')"
          >
            <Loader2 class="h-3 w-3" :class="{ 'animate-spin': props.loadingProcesses }" />
          </button>
        </div>

        <span class="whitespace-nowrap text-[10px] text-muted-foreground sm:text-sm">
          {{ props.selectedCount }} из {{ props.newCount }}
        </span>
      </div>

      <div class="flex flex-shrink-0 items-center gap-1.5">
        <button
          type="button"
          class="h-7 rounded-lg border border-border/70 px-2 text-[10px] transition-colors hover:bg-accent/70 sm:h-8 sm:px-3 sm:text-xs"
          @click="emit('selectAll', props.activeTab)"
        >
          Все
        </button>
        <button
          type="button"
          class="h-7 rounded-lg border border-border/70 px-2 text-[10px] transition-colors hover:bg-accent/70 sm:h-8 sm:px-3 sm:text-xs"
          @click="emit('deselectAll', props.activeTab)"
        >
          Сброс
        </button>
      </div>
    </div>
  </div>

  <div class="flex min-h-0 flex-1 flex-col">
    <div
      v-if="props.error"
      class="flex flex-1 flex-col items-center justify-center rounded-lg border border-destructive/20 bg-destructive/5 p-6 text-center"
    >
      <div class="mb-4 flex h-12 w-12 items-center justify-center rounded-full bg-destructive/10 text-destructive">
        <X class="h-6 w-6" />
      </div>
      <h3 class="mb-2 text-lg font-semibold text-destructive">Произошла ошибка</h3>
      <p class="max-w-md break-all rounded border bg-background/50 p-3 font-mono text-sm text-muted-foreground">
        {{ props.error }}
      </p>
      <button
        type="button"
        class="mt-4 inline-flex items-center gap-2 rounded-xl border border-border/70 px-4 py-2 text-sm transition-colors hover:bg-accent/70"
        @click="emit('loadProcesses')"
      >
        <Monitor class="h-4 w-4" />
        Попробовать снова
      </button>
    </div>

    <div
      v-else-if="props.loadingProcesses"
      class="flex flex-1 flex-col items-center justify-center rounded-lg border p-4"
    >
      <Loader2 class="mb-2 h-8 w-8 animate-spin text-primary" />
      <p class="text-xs text-muted-foreground sm:text-sm">Получение списка процессов...</p>
    </div>

    <div v-else-if="props.filteredResults.length > 0" class="flex-1 overflow-auto rounded-lg border">
      <div class="space-y-1 p-1.5 sm:p-2">
        <div
          v-for="result in props.filteredResults"
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
            @click="emit('toggleSelect', props.activeTab, result.path)"
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
                data-testid="scan-entry-name"
                placeholder="Название"
                class="-ml-1 w-fit max-w-full rounded bg-transparent px-1 text-sm font-semibold transition-colors hover:bg-accent/50 focus:outline-none sm:text-base"
                :style="{ width: `${Math.max(result.customName.length, 1)}ch` }"
                @input="
                  emit(
                    'updateName',
                    props.activeTab,
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
        v-if="props.activeTab === 'folders'"
        class="mb-4 h-10 w-10 text-muted-foreground/30 sm:h-12 sm:w-12"
      />
      <Cpu
        v-else
        class="mb-4 h-10 w-10 text-muted-foreground/30 sm:h-12 sm:w-12"
      />
      <h3 class="text-base font-medium sm:text-lg">
        {{ props.filter ? "Ничего не найдено" : "Список пуст" }}
      </h3>
      <p class="mt-2 max-w-xs text-xs text-muted-foreground sm:text-sm">
        {{
          props.filter
            ? `По запросу "${props.filter}" ничего не найдено.`
            : props.activeTab === "folders"
              ? "Выберите папку для поиска игр."
              : "Нажмите 'Обновить список' для поиска процессов."
        }}
      </p>
    </div>
  </div>

  <div
    v-if="props.selectedCount > 0"
    class="sticky bottom-0 mt-4 flex flex-col items-center justify-between gap-3 border-t bg-background/80 pt-4 backdrop-blur-sm sm:mt-6 sm:flex-row sm:pt-6"
  >
    <div class="text-xs sm:text-sm">
      Выбрано <span class="font-bold text-primary">{{ props.selectedCount }}</span>
      {{ props.selectedCount === 1 ? "игра" : "игр" }}
    </div>
    <button
      type="button"
      class="inline-flex w-full items-center justify-center gap-2 rounded-xl bg-primary px-8 py-2 text-sm font-[510] text-primary-foreground transition-colors hover:bg-accent hover:text-white sm:w-auto"
      :disabled="props.adding"
      @click="emit('addSelected', props.activeTab)"
    >
      <Loader2 v-if="props.adding" class="h-4 w-4 animate-spin" />
      <Plus v-else class="h-4 w-4" />
      Добавить
    </button>
  </div>
</template>
