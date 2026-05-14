<script setup lang="ts">
import type { RecentSessionEntry } from "@/types/analytics";
import { formatDuration, formatRelativeRange } from "@/utils/format";

defineProps<{
  sessions: RecentSessionEntry[];
}>();
</script>

<template>
  <div class="recent-sessions" data-testid="recent-sessions-table">
    <div class="recent-sessions__header recent-sessions__grid">
      <span>Приложение</span>
      <span>Устройство</span>
      <span>Интервал</span>
      <span>Фокус</span>
      <span>Простой</span>
    </div>

    <div
      v-for="session in sessions"
      :key="session.id"
      class="recent-sessions__row recent-sessions__grid"
      :data-testid="`recent-session-${session.id}`"
    >
      <div class="recent-sessions__primary">
        <span class="recent-sessions__app">{{ session.displayName }}</span>
        <span class="recent-sessions__window">{{ session.windowTitle ?? session.processName }}</span>
      </div>
      <span class="recent-sessions__secondary">{{ session.deviceName }}</span>
      <span class="recent-sessions__secondary">
        {{ formatRelativeRange(session.startedAt, session.endedAt) }}
      </span>
      <span class="recent-sessions__metric">{{ formatDuration(session.foregroundMs) }}</span>
      <span class="recent-sessions__metric">{{ formatDuration(session.idleMs) }}</span>
    </div>
  </div>
</template>

<style scoped>
.recent-sessions {
  display: flex;
  flex-direction: column;
  gap: 0.55rem;
}
.recent-sessions__grid {
  display: grid;
  grid-template-columns: minmax(0, 1.35fr) minmax(140px, 0.7fr) minmax(280px, 1.1fr) 120px 90px;
  gap: 0.9rem;
  align-items: center;
}
.recent-sessions__header {
  padding: 0 0.8rem;
  color: var(--dashboard-text-muted);
  font-size: 0.76rem;
  text-transform: uppercase;
  letter-spacing: 0.08em;
}
.recent-sessions__row {
  padding: 0.95rem 0.8rem;
  border-radius: 14px;
  background: var(--dashboard-panel-muted);
  border: 1px solid var(--border);
}
.recent-sessions__primary {
  min-width: 0;
  display: flex;
  flex-direction: column;
  gap: 0.2rem;
}
.recent-sessions__app {
  color: var(--foreground);
  font-weight: 620;
}
.recent-sessions__window,
.recent-sessions__secondary {
  color: var(--dashboard-text-soft);
  font-size: 0.84rem;
}
.recent-sessions__metric {
  color: var(--foreground);
  font-variant-numeric: tabular-nums;
  font-weight: 600;
}
@media (max-width: 1040px) {
  .recent-sessions__grid {
    grid-template-columns: minmax(0, 1fr);
  }
  .recent-sessions__header {
    display: none;
  }
}
</style>
