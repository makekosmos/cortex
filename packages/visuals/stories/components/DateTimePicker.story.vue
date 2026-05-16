<script setup lang="ts">
import { computed, ref } from "vue";
import DateTimePicker from "../../components/DateTimePicker.vue";

const empty = ref<string | null>(null);
const filled = ref<string | null>("2026-05-16T14:30:00.000Z");
const start = ref<string | null>("2026-05-16T09:00:00.000Z");
const end = ref<string | null>("2026-05-16T17:30:00.000Z");

const range = computed(() => `${start.value} → ${end.value}`);
</script>

<template>
  <Story title="DateTimePicker" group="datetime" :layout="{ type: 'single', iframe: true }">
    <Variant title="Пустое значение">
      <div class="story-canvas">
        <DateTimePicker v-model:value="empty" label="Запланировано на" />
        <p class="muted">{{ empty ?? "null" }}</p>
      </div>
    </Variant>

    <Variant title="С выбранным timestamp">
      <div class="story-canvas">
        <DateTimePicker v-model:value="filled" label="Дедлайн" />
        <p class="muted"><code>{{ filled }}</code></p>
      </div>
    </Variant>

    <Variant title="Пара 'С / По' с reference">
      <div class="story-canvas">
        <p class="story-label">
          `reference` в end-picker = start-значение → end показывает только разницу.
        </p>
        <div class="pair">
          <DateTimePicker v-model:value="start" label="С" />
          <DateTimePicker
            v-model:value="end"
            label="По"
            :reference="start"
          />
        </div>
        <p class="muted"><code>{{ range }}</code></p>
      </div>
    </Variant>
  </Story>
</template>

<style scoped>
.pair {
  display: grid;
  grid-template-columns: 1fr 1fr;
  gap: 0.75rem;
  max-width: 520px;
}
.muted {
  margin-top: 0.75rem;
  font-size: 0.85rem;
  color: var(--muted-foreground);
}
code {
  font-family: var(--font-mono);
}
</style>
