<script setup lang="ts">
import type { Component } from "vue";

interface Props {
  icon: Component;
  title: string;
  description?: string;
  imageSrc?: string;
  iconFrom?: string;
  iconTo?: string;
}

withDefaults(defineProps<Props>(), {
  description: "",
  imageSrc: "",
  iconFrom: "var(--settings-sidebar-icon-from)",
  iconTo: "var(--settings-sidebar-icon-to)",
});
</script>

<template>
  <div class="flex flex-col items-center text-center">
    <span
      class="inline-flex size-16 items-center justify-center overflow-hidden rounded-lg text-white"
      :class="
        imageSrc
          ? 'bg-none shadow-none'
          : 'bg-linear-to-bl from-[var(--settings-advanced-intro-icon-from)] to-[var(--settings-advanced-intro-icon-to)] shadow-[inset_0_0_0_1px_color-mix(in_srgb,oklch(1_0_0)_8%,transparent)]'
      "
      :style="{
        '--settings-advanced-intro-icon-from': iconFrom,
        '--settings-advanced-intro-icon-to': iconTo,
      }"
      aria-hidden="true"
    >
      <img v-if="imageSrc" class="block size-16 object-contain" :src="imageSrc" alt="" />
      <component v-else :is="icon" :size="32" :stroke-width="2" />
    </span>
    <h1
      class="mt-4 mb-0 font-[var(--font-sans)] text-[length:var(--settings-advanced-title-size)] leading-[var(--settings-advanced-title-line-height)] font-[var(--settings-advanced-title-weight)] text-[var(--foreground)]"
    >
      {{ title }}
    </h1>
    <p
      v-if="description"
      class="mt-2 mb-0 max-w-96 font-[var(--font-sans)] text-[length:var(--settings-advanced-description-size)] leading-[var(--settings-advanced-description-line-height)] font-[var(--settings-advanced-description-weight)] text-[var(--second-text-color)]"
    >
      {{ description }}
    </p>
  </div>
</template>
