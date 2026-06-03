<script setup lang="ts">
interface Props {
  className?: string;
}

defineProps<Props>();
</script>

<template>
  <div
    class="flex flex-col overflow-hidden rounded-lg bg-[var(--settings-list-background)]"
    :class="className"
  >
    <slot />
  </div>
</template>
