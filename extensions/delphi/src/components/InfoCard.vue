<script setup lang="ts"></script>

<template>
  <div class="info-card">
    <slot />
  </div>
</template>

<style scoped>
.info-card {
  margin-block: var(--space-3);
  padding: var(--space-3);
  border: 1px solid var(--border);
  border-radius: var(--radius-card);
  background: var(--card);
  color: color-mix(in srgb, var(--foreground) 70%, transparent);
  font-size: var(--kosmos-text-body-size);
  line-height: 1.5;
}
</style>
