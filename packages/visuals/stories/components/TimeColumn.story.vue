<script setup lang="ts">
import { ref } from "vue";
import TimeColumn from "../../components/TimeColumn.vue";

const hour = ref(14);
const minute = ref(30);
const seconds = ref(0);
</script>

<template>
  <Story title="TimeColumn" group="datetime">
    <Variant title="Hour + Minute (DateTimePicker compose)">
      <div class="story-canvas">
        <p class="story-label">Скролл-колонки используются внутри `DateTimePicker`.</p>
        <div class="picker">
          <TimeColumn
            v-model:value="hour"
            :min="0"
            :max="23"
            label="Часы"
          />
          <span class="sep">:</span>
          <TimeColumn
            v-model:value="minute"
            :min="0"
            :max="59"
            label="Минуты"
          />
        </div>
        <p class="muted">
          Selected:
          <code>{{ String(hour).padStart(2, "0") }}:{{ String(minute).padStart(2, "0") }}</code>
        </p>
      </div>
    </Variant>

    <Variant title="С шагом 5 минут">
      <div class="story-canvas">
        <div class="picker">
          <TimeColumn v-model:value="minute" :min="0" :max="55" :step="5" label="Минуты, шаг 5" />
        </div>
        <p class="muted">{{ minute }}</p>
      </div>
    </Variant>

    <Variant title="Секунды (0–59)">
      <div class="story-canvas">
        <div class="picker">
          <TimeColumn v-model:value="seconds" :min="0" :max="59" label="Секунды" />
        </div>
      </div>
    </Variant>
  </Story>
</template>

<style scoped>
.picker {
  display: inline-flex;
  align-items: center;
  gap: 0.25rem;
  padding: 0.5rem;
  background: var(--popover);
  border: 1px solid var(--border);
  border-radius: var(--radius);
}
.sep {
  font-size: 1.25rem;
  font-weight: 600;
  color: var(--muted-foreground);
  padding: 0 0.25rem;
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
