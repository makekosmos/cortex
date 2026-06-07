<script setup lang="ts">
// ProviderIcon — иконка AI-провайдера. SVG лежат в
// `shell/src/assets/providers/<slug>.svg` и инлайнятся через Vite `?raw`
// import. Inline даёт `fill: currentColor` cascade — иконка красится в
// цвет темы (белая на dark, чёрная на light).
//
// Чтобы добавить провайдера:
//   1. Положить `<slug>.svg` (`fill="currentColor"` на root или path) в
//      `assets/providers/`.
//   2. Использовать `<ProviderIcon provider="<slug>" />`.
// Если SVG нет — fallback на первую букву.

import { computed } from "vue";

const props = withDefaults(
  defineProps<{
    provider: string;
    size?: number;
  }>(),
  { size: 16 },
);

// `?raw` — Vite инлайнит содержимое SVG как строку.
const ICONS = import.meta.glob("/src/assets/providers/*.svg", {
  eager: true,
  query: "?raw",
  import: "default",
}) as Record<string, string>;

const svgMarkup = computed<string | null>(() => {
  const slug = props.provider.toLowerCase();
  const key = `/src/assets/providers/${slug}.svg`;
  return ICONS[key] ?? null;
});

const fallbackLetter = computed(() => props.provider.charAt(0).toUpperCase());
</script>

<template>
  <span
    class="provider-icon"
    :style="{ width: `${size}px`, height: `${size}px`, fontSize: `${Math.round(size * 0.6)}px` }"
  >
    <!-- eslint-disable-next-line vue/no-v-html — markup из bundled assets, не user-input. -->
    <span v-if="svgMarkup" class="provider-icon__svg" v-html="svgMarkup" />
    <span v-else class="provider-icon__fallback">{{ fallbackLetter }}</span>
  </span>
</template>

<style scoped>
.provider-icon {
  display: inline-flex;
  align-items: center;
  justify-content: center;
  color: var(--foreground);
  flex-shrink: 0;
}

.provider-icon__svg {
  display: inline-flex;
  width: 100%;
  height: 100%;
  /* SVG inside наследует color через fill: currentColor (в SVG: fill="currentColor"). */
  color: currentColor;
}

.provider-icon__svg :deep(svg) {
  width: 100%;
  height: 100%;
  display: block;
}

.provider-icon__fallback {
  display: inline-flex;
  align-items: center;
  justify-content: center;
  width: 100%;
  height: 100%;
  background: color-mix(in srgb, var(--foreground) 10%, transparent);
  color: color-mix(in srgb, var(--foreground) 60%, transparent);
  border-radius: 4px;
  font-weight: 600;
}
</style>
