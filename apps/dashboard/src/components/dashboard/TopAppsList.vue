<script setup lang="ts">
import type { TopAppEntry } from "@shared/analytics";
import { formatCompactHours, formatDateTimeLabel, formatSessionCount } from "@/utils/format";

defineProps<{
  items: TopAppEntry[];
}>();
</script>

<template>
  <ol class="top-apps-list" data-testid="top-apps-list">
    <li
      v-for="item in items"
      :key="item.id"
      class="top-apps-list__item"
      :data-testid="`top-app-${item.id}`"
    >
      <div class="top-apps-list__rank">{{ item.displayName.slice(0, 1) }}</div>
      <div class="top-apps-list__copy">
        <div class="top-apps-list__name">{{ item.displayName }}</div>
        <div class="top-apps-list__meta">
          <span>{{ item.processName }}</span>
          <span>{{ formatSessionCount(item.sessions) }}</span>
          <span>{{ formatDateTimeLabel(item.lastSeenAt) }}</span>
        </div>
      </div>
      <div class="top-apps-list__time">{{ formatCompactHours(item.foregroundMs) }}</div>
    </li>
  </ol>
</template>

<style scoped>
.top-apps-list { display: flex; flex-direction: column; gap: 0.6rem; margin: 0; padding: 0; list-style: none; }
.top-apps-list__item { display: grid; grid-template-columns: 40px minmax(0, 1fr) auto; gap: 0.8rem; align-items: center; padding: 0.85rem 0.9rem; border-radius: 14px; background: var(--dashboard-panel-muted); border: 1px solid var(--border); }
.top-apps-list__rank { display: grid; place-items: center; width: 40px; height: 40px; border-radius: 12px; background: color-mix(in srgb, var(--surface) 70%, transparent); color: var(--dashboard-text-soft); font-weight: 700; }
.top-apps-list__copy { min-width: 0; }
.top-apps-list__name { color: var(--foreground); font-weight: 620; }
.top-apps-list__meta { display: flex; flex-wrap: wrap; gap: 0.6rem; margin-top: 0.28rem; color: var(--dashboard-text-muted); font-size: 0.8rem; }
.top-apps-list__time { color: var(--foreground); font-variant-numeric: tabular-nums; font-size: 1rem; font-weight: 600; }
</style>
