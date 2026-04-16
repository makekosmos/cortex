<script setup lang="ts">
interface Props {
  title: string;
  caption?: string;
}

withDefaults(defineProps<Props>(), {
  caption: undefined,
});
</script>

<template>
  <section class="section-panel">
    <header class="section-panel__header">
      <div>
        <h2 class="section-panel__title">{{ title }}</h2>
        <p v-if="caption" class="section-panel__caption">{{ caption }}</p>
      </div>
      <slot name="header" />
    </header>
    <div class="section-panel__body">
      <slot />
    </div>
  </section>
</template>

<style scoped>
.section-panel {
  border: 1px solid var(--dashboard-border-strong);
  border-radius: 16px;
  background: var(--dashboard-panel);
}

.section-panel__header {
  display: flex;
  justify-content: space-between;
  gap: 1rem;
  padding: 1rem 1rem 0;
}

.section-panel__title {
  margin: 0;
  color: var(--foreground);
  font-size: 1rem;
  font-weight: 600;
  letter-spacing: -0.01em;
}

.section-panel__caption {
  margin: 0.35rem 0 0;
  color: var(--dashboard-text-soft);
  font-size: 0.86rem;
  line-height: 1.5;
}

.section-panel__body {
  padding: 1rem;
}
</style>
