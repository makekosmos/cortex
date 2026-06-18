<template>
  <div class="gsp-panel kosmos-scroll">
    <div class="gsp-title">Настройки графа</div>

    <!-- Физика -->
    <div class="gsp-section">
      <div class="gsp-section-label">Физика</div>

      <div class="gsp-row">
        <span class="gsp-row-label">Гравитация</span>
        <input
          type="range"
          min="0"
          max="0.5"
          step="0.01"
          class="gsp-slider"
          :value="settings.gravity"
          @input="settings.gravity = parseFloat(($event.target as HTMLInputElement).value)"
        />
        <span class="gsp-row-value">{{ settings.gravity.toFixed(2) }}</span>
      </div>

      <div class="gsp-row">
        <span class="gsp-row-label">Отталкивание</span>
        <input
          type="range"
          min="0.1"
          max="3"
          step="0.1"
          class="gsp-slider"
          :value="settings.repulsion"
          @input="settings.repulsion = parseFloat(($event.target as HTMLInputElement).value)"
        />
        <span class="gsp-row-value">{{ settings.repulsion.toFixed(1) }}</span>
      </div>

      <div class="gsp-row">
        <span class="gsp-row-label">Длина связей</span>
        <input
          type="range"
          min="2"
          max="60"
          step="1"
          class="gsp-slider"
          :value="settings.linkDistance"
          @input="settings.linkDistance = parseFloat(($event.target as HTMLInputElement).value)"
        />
        <span class="gsp-row-value">{{ settings.linkDistance.toFixed(0) }}</span>
      </div>

      <div class="gsp-row">
        <span class="gsp-row-label">Трение</span>
        <input
          type="range"
          min="0.5"
          max="0.98"
          step="0.01"
          class="gsp-slider"
          :value="settings.friction"
          @input="settings.friction = parseFloat(($event.target as HTMLInputElement).value)"
        />
        <span class="gsp-row-value">{{ settings.friction.toFixed(2) }}</span>
      </div>
    </div>

    <!-- Вид -->
    <div class="gsp-section">
      <div class="gsp-section-label">Вид</div>

      <div class="gsp-row">
        <span class="gsp-row-label">Размер узлов</span>
        <input
          type="range"
          min="0.3"
          max="4"
          step="0.1"
          class="gsp-slider"
          :value="settings.pointSizeScale"
          @input="settings.pointSizeScale = parseFloat(($event.target as HTMLInputElement).value)"
        />
        <span class="gsp-row-value">{{ settings.pointSizeScale.toFixed(1) }}</span>
      </div>

      <div class="gsp-row">
        <span class="gsp-row-label">Прозрачность связей</span>
        <input
          type="range"
          min="0.05"
          max="1"
          step="0.05"
          class="gsp-slider"
          :value="settings.linkOpacity"
          @input="settings.linkOpacity = parseFloat(($event.target as HTMLInputElement).value)"
        />
        <span class="gsp-row-value">{{ settings.linkOpacity.toFixed(2) }}</span>
      </div>

      <div class="gsp-row gsp-row--toggle">
        <span class="gsp-row-label">Изогнутые связи</span>
        <Toggle
          :model-value="settings.curvedLinks"
          @update:model-value="settings.curvedLinks = $event"
        />
      </div>
    </div>

    <!-- Взаимодействие -->
    <div class="gsp-section">
      <div class="gsp-section-label">Взаимодействие</div>

      <div class="gsp-row gsp-row--toggle">
        <span class="gsp-row-label">Подсветка соседей</span>
        <Toggle
          :model-value="settings.highlightNeighbors"
          @update:model-value="settings.highlightNeighbors = $event"
        />
      </div>

      <div class="gsp-row gsp-row--toggle">
        <span class="gsp-row-label">Заморозить раскладку</span>
        <Toggle :model-value="settings.frozen" @update:model-value="settings.frozen = $event" />
      </div>

      <div class="gsp-row gsp-row--toggle">
        <span class="gsp-row-label">Не давать налезать</span>
        <Toggle
          :model-value="settings.antiOverlap"
          @update:model-value="settings.antiOverlap = $event"
        />
      </div>

      <div class="gsp-row">
        <span class="gsp-row-label">Зазор</span>
        <input
          type="range"
          min="2"
          max="50"
          step="1"
          class="gsp-slider"
          :value="settings.gap"
          @input="settings.gap = parseFloat(($event.target as HTMLInputElement).value)"
        />
        <span class="gsp-row-value">{{ settings.gap.toFixed(0) }}</span>
      </div>
    </div>
  </div>
</template>

<script setup lang="ts">
import { Toggle } from "@kosmos/visuals";
import type { GraphSettings } from "./graphSettings";

defineProps<{
  settings: GraphSettings;
}>();
</script>

<style scoped>
.gsp-panel {
  -webkit-app-region: no-drag;
  background: var(--card);
  color: var(--foreground);
  border: 1px solid var(--border);
  border-radius: var(--radius-card);
  padding: var(--space-2);
  box-shadow: var(--shadow-floating);
  width: 280px;
  max-height: 70vh;
  overflow-y: auto;
  display: flex;
  flex-direction: column;
  gap: var(--space-1);
}

.gsp-title {
  font-size: var(--kosmos-text-body-size);
  font-weight: 600;
  color: var(--foreground);
  padding-bottom: var(--space-1);
  border-bottom: 1px solid var(--border);
  margin-bottom: var(--space-half, 4px);
}

.gsp-section {
  display: flex;
  flex-direction: column;
  gap: 2px;
}

.gsp-section-label {
  font-size: var(--kosmos-text-caption-size);
  color: var(--muted-foreground);
  font-weight: 600;
  text-transform: uppercase;
  letter-spacing: 0.05em;
  padding: var(--space-1) 0 2px;
}

.gsp-row {
  display: flex;
  align-items: center;
  gap: var(--space-1);
  min-height: 28px;
}

.gsp-row--toggle {
  justify-content: space-between;
}

.gsp-row-label {
  font-size: var(--kosmos-text-caption-size);
  color: var(--foreground);
  flex: 1;
  min-width: 0;
  white-space: nowrap;
  overflow: hidden;
  text-overflow: ellipsis;
}

.gsp-slider {
  flex: 1;
  accent-color: var(--accent);
  height: 4px;
  min-width: 0;
  cursor: pointer;
}

.gsp-row-value {
  font-size: var(--kosmos-text-caption-size);
  color: var(--muted-foreground);
  font-variant-numeric: tabular-nums;
  width: 36px;
  text-align: right;
  flex-shrink: 0;
}
</style>
