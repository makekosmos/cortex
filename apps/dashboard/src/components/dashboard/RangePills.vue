<script setup lang="ts">
defineProps<{
  currentRange: number;
}>();

const emit = defineEmits<{
  select: [days: number];
}>();

const options = [
  { label: "7 дн.", value: 7 },
  { label: "21 дн.", value: 21 },
  { label: "60 дн.", value: 60 },
];
</script>

<template>
  <div class="range-pills" data-testid="range-pills">
    <button
      v-for="option in options"
      :key="option.value"
      type="button"
      :class="[
        'range-pills__button',
        currentRange === option.value ? 'range-pills__button--active' : '',
      ]"
      :data-testid="`range-pill-${option.value}`"
      @click="emit('select', option.value)"
    >
      {{ option.label }}
    </button>
  </div>
</template>

<style scoped>
.range-pills { display: inline-flex; gap: 0.35rem; padding: 0.2rem; border-radius: 999px; background: var(--dashboard-panel-muted); border: 1px solid var(--border); }
.range-pills__button { padding: 0.45rem 0.8rem; border-radius: 999px; color: var(--dashboard-text-soft); font-size: 0.82rem; transition: background-color 120ms ease, color 120ms ease; }
.range-pills__button--active { background: var(--surface); color: var(--foreground); border: 1px solid var(--border); }
</style>
