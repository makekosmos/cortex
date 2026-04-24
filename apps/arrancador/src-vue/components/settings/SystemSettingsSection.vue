<script setup lang="ts">
import { Loader2, Power } from "lucide-vue-next";
import SettingsToggleRow from "./SettingsToggleRow.vue";

const props = withDefaults(
  defineProps<{
    sectionId?: string;
    autoStart: boolean;
    pending?: boolean;
  }>(),
  {
    sectionId: undefined,
    pending: false,
  },
);

const emit = defineEmits<{
  updateAutoStart: [value: boolean];
}>();
</script>

<template>
  <section :id="props.sectionId" class="space-y-4">
    <div class="flex items-center gap-2">
      <Power class="h-5 w-5" />
      <h2 class="text-base font-semibold sm:text-lg">Система</h2>
    </div>

    <div class="overflow-hidden rounded-2xl border border-border/70 bg-card/90 shadow-sm">
      <SettingsToggleRow
        id="setting-autostart"
        :model-value="props.autoStart"
        :disabled="props.pending"
        label="Запускать вместе с системой"
        description="Запускать Arrancador вместе с ОС, чтобы фоновое отслеживание было готово сразу."
        @update:model-value="emit('updateAutoStart', $event)"
      />
    </div>

    <div
      v-if="props.pending"
      class="inline-flex items-center gap-2 rounded-full border border-border/70 bg-card/70 px-3 py-1 text-xs text-muted-foreground"
    >
      <Loader2 class="h-3.5 w-3.5 animate-spin" />
      Применение параметра автозапуска...
    </div>
  </section>
</template>
