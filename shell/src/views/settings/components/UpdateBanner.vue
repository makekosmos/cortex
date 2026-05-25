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
