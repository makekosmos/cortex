<script setup lang="ts">
// UpdateBanner — Raycast-style полоса о состоянии обновления Kepler.
// Извлечён из SettingsView.vue. Сохраняет тот же DOM/класс/CSS, поэтому
// родительская страница продолжает раскладывать его внутри `.settings-shell`.
//
// Сам updateState/updateBanner поступают как пропсы — bannrer лежит над
// content и не управляет lifecycle обновлений (этим занимается owner через
// `useKeplerUpdate`).

import { ArrowUpCircle, Loader2 } from "@lucide/vue";
import type { UpdateState } from "@shared/ipc-types";
import type { UpdateBannerDescriptor } from "../composables/useKeplerUpdate";

defineProps<{
  banner: UpdateBannerDescriptor | null;
  state: UpdateState;
}>();

defineEmits<{ install: [] }>();
</script>

<template>
  <button
    v-if="banner"
    type="button"
    class="update-banner"
    :class="{ clickable: banner.clickable }"
    :disabled="!banner.clickable"
    @click="banner.clickable && $emit('install')"
  >
    <component
      :is="state.kind === 'downloading' ? Loader2 : ArrowUpCircle"
      :size="14"
      :class="{ spin: state.kind === 'downloading' }"
    />
    <span class="update-banner-text">{{ banner.text }}</span>
    <span
      v-if="banner.progress !== undefined"
      class="update-banner-progress"
      :style="{ width: `${banner.progress}%` }"
    />
  </button>
</template>

<style scoped>
/* CSS перенесён 1:1 из родительского `SettingsView.vue` scoped-style
   (lines 3427–3479 baseline). Inner-элементы UpdateBanner (`.update-banner-
   text`, `.update-banner-progress`, `.spin`) не получают `data-v-PARENT` из
   SettingsView's scope, поэтому селекторы родителя их не матчат — стили
   должны жить здесь. Корень `.update-banner` остаётся также matched
   родителем, мы дублируем правило для устойчивости. */
.update-banner {
  position: relative;
  display: flex;
  align-items: center;
  justify-content: center;
  gap: 8px;
  width: 100%;
  height: 32px;
  border: none;
  background: color-mix(in srgb, var(--accent, oklch(0.7 0.18 250)) 20%, transparent);
  color: var(--foreground);
  font: inherit;
  font-size: 12px;
  font-weight: 500;
  cursor: default;
  -webkit-app-region: no-drag;
  overflow: hidden;
}

.update-banner.clickable {
  cursor: pointer;
  background: color-mix(in srgb, var(--accent, oklch(0.7 0.18 250)) 35%, transparent);
}

.update-banner.clickable:hover {
  background: color-mix(in srgb, var(--accent, oklch(0.7 0.18 250)) 50%, transparent);
}

.update-banner-text {
  flex-shrink: 1;
  text-overflow: ellipsis;
  overflow: hidden;
  white-space: nowrap;
}

.update-banner-progress {
  position: absolute;
  bottom: 0;
  left: 0;
  height: 2px;
  background: var(--accent, oklch(0.7 0.18 250));
  transition: width 200ms ease-out;
}

.spin {
  animation: spin 1s linear infinite;
}

@keyframes spin {
  to {
    transform: rotate(360deg);
  }
}
</style>
