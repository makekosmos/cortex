<script setup lang="ts">
import type { Component } from "vue";
import { computed } from "vue";
import { RouterLink, useRoute } from "vue-router";

const props = defineProps<{
  icon: Component;
  to?: string;
  label?: string;
}>();

const emit = defineEmits<{
  click: [];
}>();

const route = useRoute();

const isActive = computed(() => (props.to ? route.path === props.to : false));

const className = computed(() => {
  const base =
    "kepler-sidebar-btn flex h-9 w-full cursor-pointer items-center gap-2.5 px-3 text-sm text-white transition-colors select-none ";
  const active = isActive.value ? "bg-white/10" : "hover:bg-white/6";
  return `${base} ${active}`;
});
</script>

<template>
  <RouterLink v-if="to" :to="to" :class="className">
    <component :is="icon" :size="18" />
    <span v-if="label" class="truncate">{{ label }}</span>
  </RouterLink>
  <button v-else type="button" :class="className" @click="emit('click')">
    <component :is="icon" :size="18" />
    <span v-if="label" class="truncate">{{ label }}</span>
  </button>
</template>

<style scoped>
.kepler-sidebar-btn {
  border-radius: calc(var(--radius) * 1.4);
  corner-shape: var(--corner-shape);
  font-weight: 500;
}
</style>
