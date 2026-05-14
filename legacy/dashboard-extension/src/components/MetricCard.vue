<script setup lang="ts">
interface Props {
  label: string;
  value: string;
  accent?: "gold" | "blue" | "mint" | "rose";
  note?: string;
}

withDefaults(defineProps<Props>(), {
  accent: "gold",
  note: undefined,
});
</script>

<template>
  <article :class="['metric-card', `metric-card--${accent}`]">
    <div class="metric-card__label">{{ label }}</div>
    <div class="metric-card__value">{{ value }}</div>
    <div v-if="note" class="metric-card__note">{{ note }}</div>
  </article>
</template>

<style scoped>
.metric-card {
  min-height: 138px;
  padding: 1rem;
  border-radius: 16px;
  border: 1px solid var(--dashboard-border-strong);
  background: var(--dashboard-panel);
}

.metric-card--gold {
  border-left: 3px solid var(--dashboard-gold);
}

.metric-card--blue {
  border-left: 3px solid var(--dashboard-blue);
}

.metric-card--mint {
  border-left: 3px solid var(--dashboard-mint);
}

.metric-card--rose {
  border-left: 3px solid var(--dashboard-rose);
}

.metric-card__label {
  color: var(--dashboard-text-muted);
  font-size: 0.78rem;
  letter-spacing: 0.06em;
  text-transform: uppercase;
}

.metric-card__value {
  margin-top: 0.65rem;
  color: var(--foreground);
  font-size: clamp(1.7rem, 2.2vw, 2.25rem);
  line-height: 1;
  letter-spacing: -0.03em;
  font-weight: 620;
}

.metric-card__note {
  margin-top: 0.75rem;
  color: var(--dashboard-text-soft);
  line-height: 1.5;
  font-size: 0.88rem;
}
</style>
