<script setup lang="ts">
import { cn } from "@/components/utils";

defineProps<{
  class?: string;
}>();
</script>

<template>
  <div
    :class="
      cn(
        'animate-pulse rounded-[var(--radius-input)] bg-[color-mix(in_srgb,var(--foreground)_10%,var(--background))]',
        $props.class,
      )
    "
  />
</template>
