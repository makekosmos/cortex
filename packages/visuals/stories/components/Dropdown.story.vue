<script setup lang="ts">
import { ref } from "vue";
import Dropdown from "../../components/Dropdown.vue";

const platform = ref<string | null>(null);
const pomodoroPreset = ref<string | null>("25");
const priority = ref<number | null>(null);
const disabledValue = ref<string | null>("medium");

const platforms = [
  { value: "windows", label: "Windows" },
  { value: "mac", label: "macOS" },
  { value: "linux", label: "Linux" },
];

const presets = [
  { value: "25", label: "25 минут", description: "Классический Pomodoro" },
  { value: "50", label: "50 минут", description: "Deep work — 50/10" },
  { value: "stopwatch", label: "Без таймера", description: "Stopwatch — свободная сессия" },
];

const priorities = [
  { value: 1, label: "Высокий" },
  { value: 2, label: "Средний" },
  { value: 3, label: "Низкий" },
  { value: 4, label: "Откладывается", disabled: true },
];

const reset = [
  { value: "low", label: "low" },
  { value: "medium", label: "medium" },
  { value: "high", label: "high" },
];
</script>

<template>
  <Story title="Dropdown" group="primitives" :layout="{ type: 'single', iframe: true }">
    <Variant title="Базовый">
      <div class="story-canvas" style="max-width:280px;">
        <p class="story-label">Generic `Dropdown&lt;T&gt;` — typed по value.</p>
        <Dropdown
          v-model="platform"
          :options="platforms"
          placeholder="Выбери платформу…"
        />
        <p class="muted">Выбрано: <code>{{ platform ?? "null" }}</code></p>
      </div>
    </Variant>

    <Variant title="С описаниями">
      <div class="story-canvas" style="max-width:320px;">
        <Dropdown v-model="pomodoroPreset" :options="presets" />
        <p class="muted">Выбрано: <code>{{ pomodoroPreset }}</code></p>
      </div>
    </Variant>

    <Variant title="С disabled-опциями (numeric value)">
      <div class="story-canvas" style="max-width:280px;">
        <Dropdown
          v-model="priority"
          :options="priorities"
          placeholder="Приоритет…"
        />
        <p class="muted">Выбрано: <code>{{ priority ?? "null" }}</code></p>
      </div>
    </Variant>

    <Variant title="Disabled trigger">
      <div class="story-canvas" style="max-width:280px;">
        <Dropdown
          v-model="disabledValue"
          :options="reset"
          :disabled="true"
        />
        <p class="muted">Триггер не реагирует на клик.</p>
      </div>
    </Variant>
  </Story>
</template>

<style scoped>
.muted {
  margin-top: 0.75rem;
  font-size: 0.8rem;
  color: var(--muted-foreground);
}
code {
  font-family: var(--font-mono);
}
</style>
