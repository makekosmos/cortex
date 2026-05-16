<script setup lang="ts">
import { spacing } from "../../tokens";

const entries = Object.entries(spacing) as [string, string][];
</script>

<template>
  <Story title="Spacing" group="tokens" :layout="{ type: 'single', iframe: false }">
    <Variant title="Шкала">
      <div class="story-canvas">
        <p class="story-label">Шкала отступов в px — соответствует `var(--spacing-*)` (не определены как CSS-vars, но используются в TS).</p>
        <div class="rows">
          <div v-for="[key, value] in entries" :key="key" class="row">
            <code class="key">{{ key }}</code>
            <span class="bar" :style="{ width: value }" />
            <code class="value">{{ value }}</code>
          </div>
        </div>
      </div>
    </Variant>
  </Story>
</template>

<style scoped>
.rows {
  display: flex;
  flex-direction: column;
  gap: 0.4rem;
}
.row {
  display: grid;
  grid-template-columns: 48px 1fr 72px;
  align-items: center;
  gap: 0.75rem;
}
.key {
  font-family: var(--font-mono);
  font-size: 0.8rem;
  color: var(--muted-foreground);
}
.bar {
  display: block;
  height: 14px;
  background: var(--accent);
  border-radius: 3px;
  min-width: 1px;
}
.value {
  font-family: var(--font-mono);
  font-size: 0.75rem;
  color: var(--foreground);
}
</style>
