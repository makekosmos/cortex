<script setup lang="ts">
import { computed } from "vue";
import type { SpaceMeta } from "./types";

const props = defineProps<{
  space: SpaceMeta;
}>();
const emit = defineEmits<{
  (e: "open", id: string): void;
}>();

const relativeTime = computed(() => {
  const diffMs = Date.now() - props.space.lastAccessedAt;
  const seconds = Math.round(diffMs / 1000);
  const minutes = Math.round(seconds / 60);
  const hours = Math.round(minutes / 60);
  const days = Math.round(hours / 24);
  const rtf = new Intl.RelativeTimeFormat("ru", { numeric: "auto" });
  if (Math.abs(seconds) < 60) return rtf.format(-seconds, "second");
  if (Math.abs(minutes) < 60) return rtf.format(-minutes, "minute");
  if (Math.abs(hours) < 24) return rtf.format(-hours, "hour");
  return rtf.format(-days, "day");
});

const countLabel = computed(() => {
  if (props.space.objectCount === null) return null;
  return `${props.space.objectCount} ОБЪЕКТОВ`;
});
</script>

<template>
  <button class="card" type="button" @click="emit('open', space.id)">
    <header class="card-header">
      <h2 class="title">{{ space.name }}</h2>
      <div v-if="countLabel" class="subtitle">{{ countLabel }}</div>
    </header>
    <footer class="card-footer">
      <span class="time">{{ relativeTime }}</span>
      <div class="pills">
        <span class="pill pill-sync" aria-label="Sync status" title="Sync status">
          <svg width="12" height="12" viewBox="0 0 24 24" fill="none"
               stroke="currentColor" stroke-width="2.5" stroke-linecap="round"
               stroke-linejoin="round" aria-hidden="true">
            <path d="M21 12a9 9 0 0 1-15 6.7L3 16" />
            <path d="M3 12a9 9 0 0 1 15-6.7L21 8" />
            <path d="M21 3v5h-5" />
            <path d="M3 21v-5h5" />
          </svg>
        </span>
        <span class="pill pill-label">{{ space.label }}</span>
      </div>
    </footer>
  </button>
</template>

<style scoped>
.card {
  position: relative;
  width: 480px;
  height: 240px;
  border-radius: 16px;
  border: 1px solid var(--border);
  background: linear-gradient(
    135deg,
    color-mix(in srgb, var(--surface) 92%, transparent) 0%,
    color-mix(in srgb, var(--surface) 70%, transparent) 100%
  );
  color: var(--foreground);
  text-align: left;
  padding: 24px;
  display: flex;
  flex-direction: column;
  justify-content: space-between;
  cursor: pointer;
  overflow: hidden;
  transition: transform 0.18s var(--easing-emphasized),
    border-color 0.18s var(--easing-standard);
}

.card::before {
  content: "";
  position: absolute;
  inset: 0;
  pointer-events: none;
  border-radius: inherit;
  background: radial-gradient(
    circle at 30% 0%,
    rgba(255, 255, 255, 0.06),
    transparent 55%
  );
}

.card:hover {
  transform: translateY(-2px);
  border-color: color-mix(in srgb, var(--foreground) 25%, transparent);
}

.card:focus-visible {
  outline: 2px solid var(--accent);
  outline-offset: 2px;
}

.card-header {
  position: relative;
}

.title {
  margin: 0;
  font-size: 26px;
  font-weight: 600;
  color: var(--foreground);
  letter-spacing: -0.4px;
}

.subtitle {
  margin-top: 6px;
  font-size: 12px;
  font-weight: 500;
  letter-spacing: 0.5px;
  text-transform: uppercase;
  color: color-mix(in srgb, var(--foreground) 50%, transparent);
}

.card-footer {
  display: flex;
  justify-content: space-between;
  align-items: flex-end;
  position: relative;
}

.time {
  font-size: 12px;
  color: color-mix(in srgb, var(--foreground) 55%, transparent);
}

.pills {
  display: inline-flex;
  gap: 8px;
}

.pill {
  display: inline-flex;
  align-items: center;
  justify-content: center;
  height: 22px;
  padding: 0 8px;
  border-radius: 10px;
  font-size: 11px;
  font-weight: 600;
  letter-spacing: 0.3px;
}

.pill-sync {
  background: var(--status-success);
  color: #0d2a13;
  padding: 0;
  width: 22px;
}

.pill-label {
  background: oklch(0.7 0.18 60);
  color: #2a1a05;
  font-family: var(--font-mono, monospace);
  font-size: 10px;
}
</style>
