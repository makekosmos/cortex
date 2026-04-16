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
  return active
    ? "kepler-sidebar-btn kepler-sidebar-btn--active"
    : "kepler-sidebar-btn";
}
</script>

<template>
  <RouterLink v-if="to" :to="to" custom v-slot="{ href, navigate, isActive }">
    <a :href="href" :class="className(active ?? isActive)" :data-testid="testId" :title="label" @click="navigate">
      <component :is="icon" :size="18" />
      <span v-if="label" class="truncate">{{ label }}</span>
    </a>
  </RouterLink>
  <button v-else type="button" :class="className(active ?? false)" :data-testid="testId" :title="label" @click="emit('click')">
    <component :is="icon" :size="18" />
    <span v-if="label" class="truncate">{{ label }}</span>
  </button>
</template>

<style scoped>
.kepler-sidebar-btn {
  display: flex;
  align-items: center;
  gap: 0.625rem;
  width: 100%;
  min-height: 2.25rem;
  padding: 0 0.75rem;
  border-radius: calc(var(--radius) * 1.4);
  corner-shape: var(--corner-shape);
  font-weight: 500;
  font-size: 0.875rem;
  line-height: 1.25rem;
  color: var(--sidebar-foreground);
  cursor: pointer;
  user-select: none;
  text-decoration: none;
  transition:
    background-color 120ms cubic-bezier(0.2, 0, 0, 1),
    color 120ms cubic-bezier(0.2, 0, 0, 1);
}

.kepler-sidebar-btn:hover {
  background: color-mix(in srgb, var(--sidebar-foreground) 6%, transparent);
}

.kepler-sidebar-btn--active {
  background: color-mix(in srgb, var(--sidebar-foreground) 10%, transparent);
}

.kepler-sidebar-btn :deep(svg) {
  flex-shrink: 0;
}

.kepler-sidebar-btn span {
  min-width: 0;
}
</style>
