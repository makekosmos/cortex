<script setup lang="ts">
import { cn } from "@/components/utils";

defineProps<{
  class?: string;
}>();
</script>

<template>
  <div :class="cn('bg-accent animate-pulse rounded-md', $props.class)" />
</template>
