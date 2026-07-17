<script setup lang="ts">
import { computed } from "vue";
import { Code2, RefreshCw } from "@lucide/vue";
import { Button, EmptyState } from "@kosmos/visuals";
import CoderActivity from "./CoderActivity.vue";
import CoderBreakdown from "./CoderBreakdown.vue";
import CoderSubmissions from "./CoderSubmissions.vue";
import { useCoderStats } from "./useCoderStats";

const { activity, error, languages, loading, recent, statuses, summary, load } = useCoderStats();

const statusLabels: Record<string, string> = {
  Accepted: "Принято",
  "Wrong Answer": "Неверный ответ",
  "Time Limit Exceeded": "Лимит времени",
  "Memory Limit Exceeded": "Лимит памяти",
  "Runtime Error": "Ошибка выполнения",
  "Compile Error": "Ошибка компиляции",
  "Output Limit Exceeded": "Лимит вывода",
  "Internal Error": "Внутренняя ошибка",
  Unknown: "Неизвестно",
};

const localizedStatuses = computed(() =>
  statuses.value.map((item) => ({ ...item, label: statusLabels[item.label] ?? item.label })),
);

function openSubmission(url: string): void {
  void window.kepler.shell.openExternal(url);
}
</script>

<template>
  <main class="coder-page kosmos-scroll">
    <header class="coder-header">
      <div>
        <h1>Кодер</h1>
        <p>Статистика задач и отправок LeetCode.</p>
      </div>
      <Button variant="ghost" size="sm" :loading="loading" @click="load">
        <template #icon><RefreshCw :size="13" /></template>
        Обновить
      </Button>
    </header>

    <EmptyState v-if="loading" title="Собираю статистику…" compact />
    <EmptyState v-else-if="error" :title="error" compact>
      <template #action><Button size="sm" @click="load">Повторить</Button></template>
    </EmptyState>
    <EmptyState
      v-else-if="summary.submissions === 0"
      title="Отправок пока нет"
      description="Подключите LeetCode в интеграциях и получите данные."
      compact
    >
      <template #icon><Code2 :size="22" /></template>
    </EmptyState>

    <template v-else>
      <section class="summary-grid" aria-label="Сводка">
        <article>
          <span>Решено</span><strong>{{ summary.solved }}</strong>
        </article>
        <article>
          <span>Отправок</span><strong>{{ summary.submissions }}</strong>
        </article>
        <article>
          <span>Принято</span><strong>{{ summary.accepted }}</strong>
        </article>
        <article>
          <span>Успешность</span><strong>{{ Math.round(summary.acceptanceRate * 100) }}%</strong>
        </article>
        <article>
          <span>Текущая серия</span><strong>{{ summary.currentStreak }} дн.</strong>
        </article>
        <article>
          <span>Лучшая серия</span><strong>{{ summary.longestStreak }} дн.</strong>
        </article>
      </section>

      <CoderActivity :days="activity" />
      <div class="breakdown-grid">
        <CoderBreakdown title="Языки" :items="languages" />
        <CoderBreakdown title="Результаты" :items="localizedStatuses" />
      </div>
      <CoderSubmissions :submissions="recent" @open="openSubmission" />
    </template>
  </main>
</template>

<style scoped>
.coder-page {
  box-sizing: border-box;
  width: 100%;
  max-width: var(--kosmos-page-max-width, 700px);
  height: 100%;
  margin: 0 auto;
  overflow-y: auto;
  padding: 20px 24px 28px;
  color: var(--foreground);
}

.coder-header {
  display: flex;
  align-items: center;
  justify-content: space-between;
  gap: 18px;
  margin-bottom: 18px;
}

.coder-header h1,
.coder-header p {
  margin: 0;
}

.coder-header h1 {
  font-size: 1.35rem;
}

.coder-header p {
  margin-top: 4px;
  color: var(--muted-foreground);
  font-size: 0.75rem;
}

.summary-grid {
  display: grid;
  grid-template-columns: repeat(6, minmax(0, 1fr));
  gap: 10px;
  margin-bottom: 14px;
}

.summary-grid article {
  display: flex;
  min-width: 0;
  flex-direction: column;
  gap: 6px;
  border: 1px solid var(--border);
  border-radius: 10px;
  padding: 13px 14px;
  background: var(--settings-list-background, var(--background));
}

.summary-grid span {
  overflow: hidden;
  color: var(--muted-foreground);
  font-size: 0.6875rem;
  line-height: 1.25;
  text-overflow: ellipsis;
}

.summary-grid strong {
  color: var(--foreground);
  font-size: 1.2rem;
}

.breakdown-grid {
  display: grid;
  grid-template-columns: repeat(2, minmax(0, 1fr));
  gap: 14px;
  margin: 14px 0;
}

@media (max-width: 900px) {
  .summary-grid {
    grid-template-columns: repeat(3, minmax(0, 1fr));
  }
}

@media (max-width: 640px) {
  .summary-grid,
  .breakdown-grid {
    grid-template-columns: 1fr;
  }
}
</style>
