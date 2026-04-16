<script setup lang="ts">
import { useDashboardData } from "@/composables/useDashboardData";
import EmptyStatePanel from "@/components/dashboard/EmptyStatePanel.vue";
import RecentSessionsTable from "@/components/dashboard/RecentSessionsTable.vue";
import SectionPanel from "@/components/dashboard/SectionPanel.vue";

const dashboard = useDashboardData();
</script>

<template>
  <div class="sessions-page" data-testid="sessions-page">
    <SectionPanel
      title="Последние сессии"
      caption="Актуальные usage-сессии, прочитанные напрямую из Ark DB без промежуточной базы трекера."
    >
      <RecentSessionsTable
        v-if="dashboard.snapshot.value && dashboard.snapshot.value.recentSessions.length > 0"
        :sessions="dashboard.snapshot.value.recentSessions"
      />
      <EmptyStatePanel
        v-else
        title="Сессии ещё не записаны"
        message="Оставьте `usage-tracker` включённым на несколько минут, и новые сессии появятся здесь."
      />
    </SectionPanel>
  </div>
</template>

<style scoped>
.sessions-page {
  display: flex;
  flex-direction: column;
  gap: 1rem;
}
</style>
