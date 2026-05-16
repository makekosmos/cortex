<script setup lang="ts">
import { typography } from "../../tokens";

const sizes = Object.entries(typography.fontSize) as [string, string][];
const weights = Object.entries(typography.fontWeight) as [string, string][];
</script>

<template>
  <Story title="Typography" group="tokens" :layout="{ type: 'single', iframe: false }">
    <Variant title="Font sizes">
      <div class="story-canvas">
        <p class="story-label">Размеры из `typography.fontSize`</p>
        <div class="stack">
          <div v-for="[key, value] in sizes" :key="key" class="row">
            <code class="meta">{{ key }} · {{ value }}</code>
            <span :style="{ fontSize: value }">Кеплер запустился — нажми ⌘K</span>
          </div>
        </div>
      </div>
    </Variant>

    <Variant title="Font weights">
      <div class="story-canvas">
        <p class="story-label">Веса из `typography.fontWeight`</p>
        <div class="stack">
          <div v-for="[key, value] in weights" :key="key" class="row">
            <code class="meta">{{ key }} · {{ value }}</code>
            <span :style="{ fontWeight: value, fontSize: '1.25rem' }">Космос — оркестратор</span>
          </div>
        </div>
      </div>
    </Variant>

    <Variant title="Семейства">
      <div class="story-canvas">
        <div class="stack">
          <div class="row">
            <code class="meta">sans</code>
            <span :style="{ fontFamily: typography.fontFamily.sans, fontSize: '1.125rem' }">
              Sans — UI / тело. Платформенный SF Pro / IBM Plex Sans / Segoe UI.
            </span>
          </div>
          <div class="row">
            <code class="meta">mono</code>
            <span :style="{ fontFamily: typography.fontFamily.mono, fontSize: '1.125rem' }">
              Mono — `ark-core-rpc::list_objects` // IBM Plex Mono → SF Mono
            </span>
          </div>
        </div>
      </div>
    </Variant>

    <Variant title="kosmos-* heading scale">
      <div class="story-canvas">
        <p class="story-label">CSS-variables `--kosmos-text-*` из `theme/css-variables.css`</p>
        <div class="stack">
          <h1 class="kosmos-page-title">Заголовок страницы</h1>
          <h2 class="kosmos-heading">Heading — section title</h2>
          <p class="kosmos-subheading">Subheading — описание под секцией</p>
          <p>Обычный body text. Inter / SF Pro / IBM Plex Sans.</p>
        </div>
      </div>
    </Variant>
  </Story>
</template>

<style scoped>
.stack {
  display: flex;
  flex-direction: column;
  gap: 1rem;
}
.row {
  display: flex;
  flex-direction: column;
  gap: 0.25rem;
}
.meta {
  font-family: var(--font-mono);
  font-size: 0.7rem;
  color: var(--muted-foreground);
  text-transform: lowercase;
}
.kosmos-page-title {
  margin: 0;
  font-size: var(--kosmos-text-page-title-size);
  line-height: var(--kosmos-text-page-title-line-height);
  letter-spacing: var(--kosmos-text-page-title-letter-spacing);
  font-weight: var(--kosmos-text-page-title-weight);
}
.kosmos-heading {
  margin: 0;
  font-size: var(--kosmos-text-heading-size);
  line-height: var(--kosmos-text-heading-line-height);
  letter-spacing: var(--kosmos-text-heading-letter-spacing);
  font-weight: var(--kosmos-text-heading-weight);
}
.kosmos-subheading {
  margin: 0;
  font-size: var(--kosmos-text-subheading-size);
  line-height: var(--kosmos-text-subheading-line-height);
  letter-spacing: var(--kosmos-text-subheading-letter-spacing);
  font-weight: var(--kosmos-text-subheading-weight);
  color: var(--muted-foreground);
}
</style>
