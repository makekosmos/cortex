<script setup lang="ts">
import { ref } from "vue";
import DateChip from "../../components/DateChip.vue";

const empty = ref<string | null>(null);
const active = ref<string | null>("2026-05-16");
const compact = ref<string | null>("2026-12-31");
</script>

<template>
  <Story title="DateChip" group="datetime">
    <Variant title="Состояния">
      <div class="story-canvas">
        <p class="story-label">Кликни — раскроется Calendar popover.</p>
        <div class="story-row">
          <div class="story-col">
            <span class="story-label">Пустой</span>
            <DateChip v-model:value="empty" />
          </div>
          <div class="story-col">
            <span class="story-label">С датой</span>
            <DateChip v-model:value="active" />
          </div>
          <div class="story-col">
            <span class="story-label">Compact</span>
            <DateChip v-model:value="compact" :compact="true" />
          </div>
        </div>
      </div>
    </Variant>

    <Variant title="Кастомный placeholder">
      <div class="story-canvas">
        <DateChip v-model:value="empty" placeholder="Запланировать…" />
      </div>
    </Variant>
  </Story>
</template>
