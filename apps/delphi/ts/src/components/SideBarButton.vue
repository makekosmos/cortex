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
    "flex h-10 w-full cursor-pointer items-center gap-2.5 rounded-lg px-3 py-2 text-sm transition-colors";
  const active = isActive.value
    ? "bg-(--accent) text-(--foreground) font-medium"
    : "text-(--muted-foreground) hover:bg-(--secondary) hover:text-(--foreground)";
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
