<script setup lang="ts">
interface Props {
  className?: string;
}

defineProps<Props>();
</script>

<template>
  <div class="kosmos-settings-list" :class="className">
    <slot />
  </div>
</template>

<style scoped>
.kosmos-settings-list {
  display: flex;
  flex-direction: column;
  background: var(--settings-list-background);
  border-radius: 12px;
  overflow: hidden;
}
</style>
