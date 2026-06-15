<script setup lang="ts">
defineOptions({
  inheritAttrs: false,
});

const props = defineProps<{
  class?: string;
}>();
</script>

<template>
  <div
    v-bind="$attrs"
    :class="[
      'kosmos-skeleton rounded-md bg-[color-mix(in_srgb,var(--foreground)_10%,transparent)]',
      props.class,
    ]"
    aria-hidden="true"
  />
</template>

<style scoped>
.kosmos-skeleton {
  animation: kosmos-skeleton-pulse 1.45s ease-in-out infinite;
}

@keyframes kosmos-skeleton-pulse {
  0%,
  100% {
    opacity: 1;
  }

  50% {
    opacity: 0.52;
  }
}
</style>
