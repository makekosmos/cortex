<script setup lang="ts">
import { HardDrive } from "lucide-vue-next";
import SettingsToggleRow from "./SettingsToggleRow.vue";

const props = withDefaults(
  defineProps<{
    sectionId?: string;
    compressionEnabled: boolean;
    compressionLevel: number;
    skipCompressionOnce: boolean;
  }>(),
  {
    sectionId: undefined,
  },
);

const emit = defineEmits<{
  updateCompressionEnabled: [value: boolean];
  updateCompressionLevel: [value: number];
  updateSkipCompressionOnce: [value: boolean];
}>();

function handleCompressionInput(event: Event) {
  const value = Number.parseInt((event.target as HTMLInputElement).value, 10);
  if (Number.isNaN(value)) {
    return;
  }

  emit("updateCompressionLevel", value);
}
</script>

<template>
  <section :id="props.sectionId" class="space-y-4">
    <div class="flex items-center gap-2">
      <HardDrive class="h-5 w-5" />
      <h2 class="text-base font-semibold sm:text-lg">SQOBA Compression</h2>
    </div>

    <div class="space-y-4 rounded-2xl border border-border/70 bg-card/90 p-4 shadow-sm">
      <SettingsToggleRow
        id="setting-compression"
        :model-value="props.compressionEnabled"
        label="Enable compression"
        description="Compressed backups save space while keeping backup history manageable."
        @update:model-value="emit('updateCompressionEnabled', $event)"
      />

      <div class="space-y-3" :class="props.compressionEnabled ? '' : 'opacity-50'">
        <div class="flex items-center gap-3">
          <label class="text-xs text-muted-foreground" for="compression-level">
            Level
          </label>
          <input
            id="compression-level"
            type="number"
            min="1"
            max="100"
            :value="props.compressionLevel"
            :disabled="!props.compressionEnabled"
            class="h-10 w-20 rounded-xl border border-border/70 bg-card/70 px-3 text-sm outline-none disabled:cursor-not-allowed"
            @input="handleCompressionInput"
          />
          <div class="ml-auto text-xs text-muted-foreground">
            {{ props.compressionLevel }}
          </div>
        </div>

        <input
          type="range"
          min="1"
          max="100"
          :value="props.compressionLevel"
          :disabled="!props.compressionEnabled"
          class="w-full accent-primary disabled:cursor-not-allowed"
          @input="handleCompressionInput"
        />

        <p class="text-xs text-muted-foreground">
          Lower values are faster. Higher values are smaller. A balanced default is usually 40-70.
        </p>
      </div>

      <SettingsToggleRow
        id="setting-skip-compression"
        :model-value="props.skipCompressionOnce"
        :disabled="!props.compressionEnabled"
        label="Skip compression once"
        description="The next backup will be created uncompressed, then this flag resets after use."
        @update:model-value="emit('updateSkipCompressionOnce', $event)"
      />
    </div>
  </section>
</template>
