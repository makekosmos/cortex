<script setup lang="ts">
// AboutTab — версия Kepler + кнопка «Проверить обновления».

import AdvancedPageLayout, {
  type IntroDescriptor,
} from "../components/AdvancedPageLayout.vue";
import LegacyRow from "../components/LegacyRow.vue";

defineProps<{
  intro: IntroDescriptor | null;
  version: string;
  checkResultLabel: string;
  updateChecking: boolean;
  isDownloading: boolean;
}>();

defineEmits<{ checkUpdates: [] }>();
</script>

<template>
  <AdvancedPageLayout :intro="intro" body-class="rows">
    <LegacyRow title="Версия Kepler" :hint="checkResultLabel">
      <div class="row-actions">
        <code class="value">{{ version }}</code>
        <button
          type="button"
          class="btn ghost"
          :disabled="updateChecking || isDownloading"
          @click="$emit('checkUpdates')"
        >
          {{ updateChecking ? "Проверяем…" : "Проверить обновления" }}
        </button>
      </div>
    </LegacyRow>
  </AdvancedPageLayout>
</template>
