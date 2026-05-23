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
  <div class="kosmos-settings-advanced-intro">
    <span
      class="kosmos-settings-advanced-intro__icon"
      :class="{ 'kosmos-settings-advanced-intro__icon--image': imageSrc }"
      :style="{
        '--settings-advanced-intro-icon-from': iconFrom,
        '--settings-advanced-intro-icon-to': iconTo,
      }"
      aria-hidden="true"
    >
      <img v-if="imageSrc" class="kosmos-settings-advanced-intro__image" :src="imageSrc" alt="" />
      <component v-else :is="icon" :size="28" :stroke-width="2" />
    </span>
    <h1 class="kosmos-settings-advanced-intro__title">{{ title }}</h1>
    <p v-if="description" class="kosmos-settings-advanced-intro__description">
      {{ description }}
    </p>
  </div>
</template>

<style scoped>
.kosmos-settings-advanced-intro {
  display: flex;
  align-items: center;
  flex-direction: column;
  text-align: center;
}

.kosmos-settings-advanced-intro__icon {
  display: inline-flex;
  width: 52px;
  height: 52px;
  align-items: center;
  justify-content: center;
  border-radius: 10px;
  background-image: linear-gradient(
    to bottom left,
    var(--settings-advanced-intro-icon-from),
    var(--settings-advanced-intro-icon-to)
  );
  box-shadow: inset 0 0 0 1px color-mix(in srgb, oklch(1 0 0) 8%, transparent);
  color: #fff;
  overflow: hidden;
}

.kosmos-settings-advanced-intro__icon--image {
  background-image: none;
  box-shadow: none;
}

.kosmos-settings-advanced-intro__image {
  display: block;
  width: 52px;
  height: 52px;
  object-fit: contain;
}

.kosmos-settings-advanced-intro__title {
  margin: 14px 0 0;
  color: var(--foreground);
  font-family: var(--font-sans);
  font-size: var(--settings-advanced-title-size);
  line-height: var(--settings-advanced-title-line-height);
  font-weight: var(--settings-advanced-title-weight);
}

.kosmos-settings-advanced-intro__description {
  max-width: 360px;
  margin: 6px 0 0;
  color: var(--second-text-color);
  font-family: var(--font-sans);
  font-size: var(--settings-advanced-description-size);
  line-height: var(--settings-advanced-description-line-height);
  font-weight: var(--settings-advanced-description-weight);
}
</style>
