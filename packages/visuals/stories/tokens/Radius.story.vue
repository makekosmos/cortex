<script setup lang="ts">
import { radius } from "../../tokens";

const entries = Object.entries(radius) as [string, string][];

const semanticVars = [
  { name: "--radius", description: "Базовый радиус (0.85rem)" },
  { name: "--radius-card", description: "Большие card-обёртки — calc(--radius * 3)" },
  { name: "--radius-button", description: "Primary CTA pills — calc(--radius * 1.5)" },
  { name: "--radius-input", description: "Inputs/dropdowns/secondary — calc(--radius * 0.7)" },
];
</script>

<template>
  <Story title="Radius" group="tokens" :layout="{ type: 'single', iframe: false }">
    <Variant title="Шкала">
      <div class="story-canvas">
        <p class="story-label">TS-токены `radius.*` (для inline-стилей)</p>
        <div class="story-row">
          <div v-for="[key, value] in entries" :key="key" class="card">
            <div class="box" :style="{ borderRadius: value }" />
            <code class="meta">{{ key }}</code>
            <code class="meta muted">{{ value }}</code>
          </div>
        </div>
      </div>
    </Variant>

    <Variant title="Семантические CSS-vars">
      <div class="story-canvas">
        <p class="story-label">Используй эти, не «магические» значения</p>
        <div class="story-col">
          <div v-for="v in semanticVars" :key="v.name" class="semantic-row">
            <div class="box semantic" :style="{ borderRadius: `var(${v.name})` }" />
            <div>
              <code>{{ v.name }}</code>
              <p class="muted">{{ v.description }}</p>
            </div>
          </div>
        </div>
      </div>
    </Variant>
  </Story>
</template>

<style scoped>
.card {
  display: flex;
  flex-direction: column;
  align-items: center;
  gap: 0.35rem;
}
.box {
  width: 64px;
  height: 64px;
  background: var(--accent);
  border: 1px solid var(--border);
}
.semantic-row {
  display: flex;
  align-items: center;
  gap: 1rem;
}
.semantic {
  width: 80px;
  height: 48px;
  background: var(--secondary);
}
.meta {
  font-family: var(--font-mono);
  font-size: 0.75rem;
}
.muted {
  color: var(--muted-foreground);
}
p.muted {
  margin: 0.2rem 0 0;
  font-size: 0.825rem;
}
</style>
