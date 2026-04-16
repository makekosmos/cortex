<script setup lang="ts">
defineProps<{
  title: string;
  message: string;
}>();
</script>

<template>
  <div class="empty-state-panel">
    <div class="empty-state-panel__title">{{ title }}</div>
    <div class="empty-state-panel__message">{{ message }}</div>
  </div>
</template>

<style scoped>
.empty-state-panel { padding: 1rem; border-radius: 14px; border: 1px solid var(--border); background: var(--dashboard-panel-muted); color: var(--dashboard-text-soft); }
.empty-state-panel__title { font-weight: 620; color: var(--foreground); }
.empty-state-panel__message { margin-top: 0.45rem; line-height: 1.6; }
</style>
