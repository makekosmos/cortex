<script setup lang="ts">
import { computed, onMounted, reactive } from "vue";
import { Cpu, HardDrive, Loader2, MemoryStick, Monitor, RefreshCw, Video } from "lucide-vue-next";
import { systemApi } from "../../src/lib/api";
import type { DiskSpeedResult, SystemInfo } from "../../src/types";

const STORAGE_KEY = "arrancador_system_info_snapshot";
const STORAGE_CHECKED_KEY = "arrancador_system_info_checked_at";

const state = reactive<{
  info: SystemInfo | null;
  checkedAt: string | null;
  loading: boolean;
  error: string | null;
  diskTests: Record<string, { loading: boolean; result?: DiskSpeedResult; error?: string }>;
}>({
  info: null,
  checkedAt: null,
  loading: false,
  error: null,
  diskTests: {},
});

const formatBytes = (value: number) => {
  if (!value || value <= 0) return "0 B";
  const units = ["B", "KB", "MB", "GB", "TB"];
  let size = value;
  let index = 0;
  while (size >= 1024 && index < units.length - 1) {
    size /= 1024;
    index += 1;
  }
  return `${size.toFixed(size >= 10 ? 0 : 1)} ${units[index]}`;
};

const formatSpeed = (value: number) => `${Math.round(value)} MB/s`;
const formatCheckedAt = (value: string | null) => {
  if (!value) return "—";
  const parsed = new Date(value);
  return Number.isNaN(parsed.getTime()) ? "—" : parsed.toLocaleString("ru-RU");
};
const formatCoreLabel = (physical: number | null, logical: number) =>
  physical ? `${physical} физ. / ${logical} лог.` : logical ? `${logical} лог.` : "—";

const memoryUsagePercent = computed(() => {
  if (!state.info || state.info.memory.total_bytes <= 0) return 0;
  return Math.min(100, (state.info.memory.used_bytes / state.info.memory.total_bytes) * 100);
});

async function refresh() {
  state.loading = true;
  state.error = null;

  try {
    const info = await systemApi.getInfo();
    const checkedAt = new Date().toISOString();
    state.info = info;
    state.checkedAt = checkedAt;
    window.localStorage.setItem(STORAGE_KEY, JSON.stringify(info));
    window.localStorage.setItem(STORAGE_CHECKED_KEY, checkedAt);
  } catch (cause) {
    console.error("Failed to load system info:", cause);
    state.error = "Не удалось загрузить данные о системе";
  } finally {
    state.loading = false;
  }
}

async function runDiskTest(mountPoint: string) {
  state.diskTests[mountPoint] = { loading: true };
  try {
    const result = await systemApi.testDiskSpeed(mountPoint);
    state.diskTests[mountPoint] = { loading: false, result };
  } catch (cause) {
    console.error("Failed to test disk speed:", cause);
    state.diskTests[mountPoint] = { loading: false, error: "Не удалось провести тест" };
  }
}

onMounted(() => {
  const cached = window.localStorage.getItem(STORAGE_KEY);
  const cachedCheckedAt = window.localStorage.getItem(STORAGE_CHECKED_KEY);

  if (cached) {
    try {
      state.info = JSON.parse(cached) as SystemInfo;
    } catch (cause) {
      console.warn("Failed to parse cached system info:", cause);
      window.localStorage.removeItem(STORAGE_KEY);
    }
  }

  state.checkedAt = cachedCheckedAt;
});
</script>

<template>
  <div class="space-y-6 p-6">
    <div class="flex flex-wrap items-center justify-between gap-3">
      <div>
        <h1 class="text-2xl font-semibold tracking-tight">Система</h1>
        <p class="mt-1 text-sm text-muted-foreground">
          Последняя проверка: {{ formatCheckedAt(state.checkedAt) }}
        </p>
      </div>
      <button
        type="button"
        class="inline-flex h-10 items-center gap-2 rounded-full border border-border/70 bg-card/80 px-4 text-sm font-[510] transition-colors hover:bg-accent/70"
        :disabled="state.loading"
        @click="refresh"
      >
        <Loader2 v-if="state.loading" class="h-4 w-4 animate-spin" />
        <RefreshCw v-else class="h-4 w-4" />
        Обновить
      </button>
    </div>

    <div
      v-if="state.error"
      class="rounded-xl border border-red-500/30 bg-red-500/10 px-4 py-3 text-sm text-red-300"
    >
      {{ state.error }}
    </div>

    <div v-if="state.info" class="grid gap-4 xl:grid-cols-2">
      <section class="rounded-2xl border border-border/70 bg-card/80 p-5 shadow-[0_12px_30px_rgba(0,0,0,0.18)]">
        <div class="mb-4 flex items-center gap-2 text-sm font-[510]">
          <Cpu class="h-4 w-4" />
          CPU
        </div>
        <div class="space-y-3 text-sm">
          <div class="flex items-center justify-between gap-4">
            <span class="text-muted-foreground">Модель</span>
            <span class="text-right font-medium">{{ state.info.cpu.brand || "—" }}</span>
          </div>
          <div class="flex items-center justify-between gap-4">
            <span class="text-muted-foreground">Ядра</span>
            <span class="font-medium">{{ formatCoreLabel(state.info.cpu.physical_cores, state.info.cpu.logical_cores) }}</span>
          </div>
          <div class="flex items-center justify-between gap-4">
            <span class="text-muted-foreground">Частота</span>
            <span class="font-medium">{{ state.info.cpu.frequency_mhz }} MHz</span>
          </div>
        </div>
      </section>

      <section class="rounded-2xl border border-border/70 bg-card/80 p-5 shadow-[0_12px_30px_rgba(0,0,0,0.18)]">
        <div class="mb-4 flex items-center gap-2 text-sm font-[510]">
          <MemoryStick class="h-4 w-4" />
          Память
        </div>
        <div class="space-y-3 text-sm">
          <div class="flex items-center justify-between gap-4">
            <span class="text-muted-foreground">Использовано</span>
            <span class="font-medium">{{ formatBytes(state.info.memory.used_bytes) }}</span>
          </div>
          <div class="h-2 overflow-hidden rounded-full border border-border/60 bg-white/6">
            <div
              class="h-full bg-primary transition-all"
              :style="{ width: `${memoryUsagePercent}%` }"
            />
          </div>
          <div class="flex items-center justify-between gap-4">
            <span class="text-muted-foreground">Всего</span>
            <span class="font-medium">{{ formatBytes(state.info.memory.total_bytes) }}</span>
          </div>
        </div>
      </section>

      <section class="rounded-2xl border border-border/70 bg-card/80 p-5 shadow-[0_12px_30px_rgba(0,0,0,0.18)]">
        <div class="mb-4 flex items-center gap-2 text-sm font-[510]">
          <Video class="h-4 w-4" />
          GPU
        </div>
        <div class="space-y-3 text-sm">
          <div
            v-for="gpu in state.info.gpus"
            :key="gpu.name"
            class="flex items-center justify-between gap-4"
          >
            <span class="text-muted-foreground">{{ gpu.vendor || "GPU" }}</span>
            <span class="text-right font-medium">{{ gpu.name }}</span>
          </div>
        </div>
      </section>

      <section class="rounded-2xl border border-border/70 bg-card/80 p-5 shadow-[0_12px_30px_rgba(0,0,0,0.18)]">
        <div class="mb-4 flex items-center gap-2 text-sm font-[510]">
          <Monitor class="h-4 w-4" />
          ОС
        </div>
        <div class="space-y-3 text-sm">
          <div class="flex items-center justify-between gap-4">
            <span class="text-muted-foreground">Платформа</span>
            <span class="font-medium">{{ state.info.os.platform }}</span>
          </div>
          <div class="flex items-center justify-between gap-4">
            <span class="text-muted-foreground">Версия</span>
            <span class="font-medium">{{ state.info.os.version }}</span>
          </div>
        </div>
      </section>
    </div>

    <section
      v-if="state.info"
      class="rounded-2xl border border-border/70 bg-card/80 p-5 shadow-[0_12px_30px_rgba(0,0,0,0.18)]"
    >
      <div class="mb-4 flex items-center gap-2 text-sm font-[510]">
        <HardDrive class="h-4 w-4" />
        Накопители
      </div>
      <div class="space-y-4">
        <div
          v-for="disk in state.info.disks"
          :key="disk.mount_point"
          class="rounded-xl border border-border/60 bg-background/40 p-4"
        >
          <div class="flex flex-wrap items-start justify-between gap-3">
            <div>
              <div class="text-sm font-medium">{{ disk.mount_point }}</div>
              <div class="mt-1 text-xs text-muted-foreground">
                {{ formatBytes(disk.available_bytes) }} свободно из {{ formatBytes(disk.total_bytes) }}
              </div>
            </div>
            <button
              type="button"
              class="inline-flex h-9 items-center gap-2 rounded-full border border-border/70 bg-card/80 px-3 text-sm font-[510] transition-colors hover:bg-accent/70"
              :disabled="state.diskTests[disk.mount_point]?.loading"
              @click="runDiskTest(disk.mount_point)"
            >
              <Loader2
                v-if="state.diskTests[disk.mount_point]?.loading"
                class="h-4 w-4 animate-spin"
              />
              <RefreshCw v-else class="h-4 w-4" />
              Тест
            </button>
          </div>

          <div
            v-if="state.diskTests[disk.mount_point]?.result"
            class="mt-3 grid gap-2 text-sm sm:grid-cols-2"
          >
            <div>Чтение: {{ formatSpeed(state.diskTests[disk.mount_point].result!.read_mb_s) }}</div>
            <div>Запись: {{ formatSpeed(state.diskTests[disk.mount_point].result!.write_mb_s) }}</div>
          </div>
          <div
            v-else-if="state.diskTests[disk.mount_point]?.error"
            class="mt-3 text-sm text-red-300"
          >
            {{ state.diskTests[disk.mount_point].error }}
          </div>
        </div>
      </div>
    </section>
  </div>
</template>
