<script setup lang="ts">
// AdvancedPageLayout — стандартная обвязка advanced-таба:
// `<section class="advanced-page kosmos-scroll">` + опциональный
// `SettingsAdvancedIntro` херо-блок сверху + slot для тела.
//
// Использовался идентично в 5 tab'ах (Dictation/Focus/Extensions/
// FileSearch/AppCommands). Каждое использование стоило ~10 строк
// boilerplate'а — теперь 1 строка `<AdvancedPageLayout :intro="...">`.

import { SettingsAdvancedIntro } from "@kosmos/visuals";
import type { Component } from "vue";

export interface IntroDescriptor {
  icon: Component;
  label: string;
  description?: string;
  introImage?: string;
  iconGradient?: { from?: string; to?: string };
}

defineProps<{
  intro: IntroDescriptor | null;
  pageClass?: string;
  /** Не используется по умолчанию (`.advanced-page__body` flex column).
   * Если tab'у нужен другой layout — переопределите через class на default slot. */
  bodyClass?: string;
}>();
</script>

<template>
  <section class="advanced-page kosmos-scroll" :class="pageClass">
    <SettingsAdvancedIntro
      v-if="intro"
      :icon="intro.icon"
      :title="intro.label"
      :description="intro.description"
      :image-src="intro.introImage"
      :icon-from="intro.iconGradient?.from"
      :icon-to="intro.iconGradient?.to"
    />
    <div class="advanced-page__body" :class="bodyClass">
      <slot />
    </div>
  </section>
</template>
