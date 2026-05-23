<script setup lang="ts">
import type { Component } from "vue";

interface Props {
  icon: Component;
  label: string;
  active?: boolean;
  iconImage?: string;
  iconFrom?: string;
  iconTo?: string;
}

withDefaults(defineProps<Props>(), {
  active: false,
  iconImage: "",
  iconFrom: "var(--settings-sidebar-icon-from)",
  iconTo: "var(--settings-sidebar-icon-to)",
});

defineEmits<{
  click: [];
}>();
</script>

<template>
  <button
    type="button"
    class="kosmos-settings-sidebar-button"
    :class="{ 'kosmos-settings-sidebar-button--active': active }"
    @click="$emit('click')"
  >
    <span
      class="kosmos-settings-sidebar-button__icon"
      :class="{ 'kosmos-settings-sidebar-button__icon--image': iconImage }"
      :style="{
        '--settings-sidebar-button-icon-from': iconFrom,
        '--settings-sidebar-button-icon-to': iconTo,
      }"
      aria-hidden="true"
    >
      <img v-if="iconImage" class="kosmos-settings-sidebar-button__image" :src="iconImage" alt="" />
      <component v-else :is="icon" :size="14" :stroke-width="2" />
    </span>
    <span class="kosmos-settings-sidebar-button__label">{{ label }}</span>
  </button>
</template>

<style scoped>
.kosmos-settings-sidebar-button {
  display: flex;
  width: 212px;
  align-items: center;
  gap: 10px;
  padding: 5px 6px;
  border: none;
  border-radius: 6px;
  background: transparent;
  color: #fff;
  cursor: default;
  text-align: left;
}

.kosmos-settings-sidebar-button--active {
  background: var(--settings-sidebar-active);
}

.kosmos-settings-sidebar-button__icon {
  display: inline-flex;
  width: 22px;
  height: 22px;
  align-items: center;
  justify-content: center;
  flex-shrink: 0;
  border-radius: 4px;
  background-image: linear-gradient(
    to bottom left,
    var(--settings-sidebar-button-icon-from),
    var(--settings-sidebar-button-icon-to)
  );
  box-shadow: inset 0 0 0 1px color-mix(in srgb, oklch(1 0 0) 6%, transparent);
  color: #fff;
  overflow: hidden;
}

.kosmos-settings-sidebar-button__icon--image {
  background-image: none;
  box-shadow: none;
}

.kosmos-settings-sidebar-button__image {
  display: block;
  width: 22px;
  height: 22px;
  object-fit: contain;
}

.kosmos-settings-sidebar-button__label {
  min-width: 0;
  flex: 1;
  overflow: hidden;
  text-overflow: ellipsis;
  white-space: nowrap;
  font-family: var(--font-sans);
  font-size: 13px;
  line-height: 1.4;
  font-weight: 500;
}
</style>
