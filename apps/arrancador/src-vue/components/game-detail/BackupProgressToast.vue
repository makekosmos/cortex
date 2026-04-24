<script setup lang="ts">
import { Loader2 } from "lucide-vue-next";
import { computed } from "vue";

const props = defineProps<{
  progress: {
    active: boolean;
    stage: string;
    message: string;
    done: number;
    total: number;
  };
}>();

const title = computed(() => {
  if (props.progress.stage === "scan") return "Подготовка бэкапа";
  if (props.progress.stage === "copy") return "Создание бэкапа";
  if (props.progress.stage === "restore") return "Восстановление бэкапа";
  return "Обработка";
});
</script>

<template>
  <div v-if="progress.active" class="fixed right-4 bottom-4 left-4 z-[124] lg:left-[300px]">
    <div class="flex items-center gap-3 rounded-xl border border-border/60 bg-card/80 px-4 py-3 shadow-[0_16px_40px_rgba(8,12,24,0.45)] backdrop-blur-xl">
      <Loader2 class="h-4 w-4 animate-spin text-primary" />
      <div class="min-w-0">
        <div class="text-sm font-medium">
          {{ title }}
        </div>
        <div class="truncate text-xs text-muted-foreground">
          {{ progress.message }}
        </div>
      </div>
      <div v-if="progress.total > 0" class="ml-auto text-xs tabular-nums text-muted-foreground">
        {{ progress.done }}/{{ progress.total }}
      </div>
    </div>
  </div>
</template>
