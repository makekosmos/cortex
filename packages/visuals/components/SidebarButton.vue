<script setup lang="ts">
import type { Component } from "vue";
import { RouterLink } from "vue-router";

const props = defineProps<{
  icon: Component;
  to?: string;
  label?: string;
  active?: boolean;
  testId?: string;
}>();

const emit = defineEmits<{
  click: [];
}>();

function className(active: boolean) {
  const base =
    "kosmos-sidebar-btn flex min-h-8 w-full items-center justify-start gap-2 rounded-lg p-2 text-left text-sm leading-5 font-medium text-(--sidebar-foreground) no-underline select-none transition-[background-color,color] duration-[120ms] ease-[cubic-bezier(0.2,0,0,1)] [corner-shape:var(--corner-shape)] hover:bg-[color-mix(in_srgb,var(--sidebar-foreground)_6%,transparent)] [&_svg]:shrink-0";
  return active
    ? `${base} bg-[color-mix(in_srgb,var(--sidebar-foreground)_10%,transparent)]`
    : base;
}
</script>

<template>
  <RouterLink v-if="to" :to="to" custom v-slot="{ href, navigate, isActive }">
    <a
      :href="href"
      :class="className(active ?? isActive)"
      :data-testid="testId"
      :title="label"
      @click="navigate"
    >
      <component :is="icon" :size="24" />
      <span v-if="label" class="min-w-0 flex-1 truncate text-left">{{ label }}</span>
    </a>
  </RouterLink>
  <button
    v-else
    type="button"
    :class="className(active ?? false)"
    :data-testid="testId"
    :title="label"
    @click="emit('click')"
  >
    <component :is="icon" :size="24" />
    <span v-if="label" class="min-w-0 flex-1 truncate text-left">{{ label }}</span>
  </button>
</template>

<style scoped>
.kosmos-sidebar-btn {
  -webkit-app-region: no-drag;
}
</style>
