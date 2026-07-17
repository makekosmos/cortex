<script setup lang="ts">
import type { CoderBreakdownItem } from "./types";

defineProps<{ title: string; items: CoderBreakdownItem[] }>();
</script>

<template>
  <section class="breakdown-panel">
    <h2>{{ title }}</h2>
    <div v-if="items.length" class="breakdown-list">
      <div v-for="item in items" :key="item.label" class="breakdown-row">
        <div class="breakdown-label">
          <span>{{ item.label }}</span>
          <strong>{{ item.count }}</strong>
        </div>
        <div class="breakdown-track">
          <span :style="{ width: `${Math.max(2, item.share * 100)}%` }" />
        </div>
      </div>
    </div>
    <p v-else>Пока нет данных</p>
  </section>
</template>

<style scoped>
.breakdown-panel {
  min-width: 0;
  border: 1px solid var(--border);
  border-radius: 10px;
  padding: 16px;
  background: var(--settings-list-background, var(--background));
}

h2,
p {
  margin: 0;
}

h2 {
  color: var(--foreground);
  font-size: 0.875rem;
}

p {
  margin-top: 14px;
  color: var(--muted-foreground);
  font-size: 0.6875rem;
}

.breakdown-list {
  display: flex;
  flex-direction: column;
  gap: 12px;
  margin-top: 14px;
}

.breakdown-label {
  display: flex;
  justify-content: space-between;
  gap: 10px;
  color: var(--foreground);
  font-size: 0.75rem;
}

.breakdown-label strong {
  color: var(--muted-foreground);
  font-size: 0.6875rem;
}

.breakdown-track {
  height: 4px;
  margin-top: 6px;
  overflow: hidden;
  border-radius: 999px;
  background: color-mix(in srgb, var(--foreground) 7%, transparent);
}

.breakdown-track span {
  display: block;
  height: 100%;
  border-radius: inherit;
  background: var(--accent);
}
</style>
