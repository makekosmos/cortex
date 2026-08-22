<script setup lang="ts">
// DebugTab — launcher TTL.

import LegacyRow from "../components/LegacyRow.vue";

defineProps<{
  launcherStateTtl: number;
}>();

defineEmits<{
  launcherStateTtlChange: [e: Event];
}>();
</script>

<template>
  <div class="rows kosmos-scroll">
    <LegacyRow title="Запоминать позицию в лаунчере">
      <template #hint>
        Сохраняет введённый текст, выбранный пункт и прокрутку между открытиями лаунчера.
        <code>0</code> — всегда ресетить при открытии.
      </template>
      <label class="ttl-input">
        <input
          type="number"
          min="0"
          max="1440"
          step="1"
          :value="launcherStateTtl"
          @change="(e: Event) => $emit('launcherStateTtlChange', e)"
        />
        <span>мин</span>
      </label>
    </LegacyRow>
  </div>
</template>
