<script setup lang="ts">
import { computed, shallowRef } from "vue";
import { Code2, RefreshCw } from "@lucide/vue";
import { Button, EmptyState } from "@kosmos/visuals";
import CoderActivity from "./CoderActivity.vue";
import CoderBreakdown from "./CoderBreakdown.vue";
import CoderDifficultyGauge from "./CoderDifficultyGauge.vue";
import CoderSubmissions from "./CoderSubmissions.vue";
import { useCoderStats } from "./useCoderStats";

const {
  activity,
  difficulties,
  error,
  languages,
  loading,
  recent,
  refreshing,
  statuses,
  summary,
  load,
  refresh,
} = useCoderStats();
const platform = shallowRef<"leetcode" | "codewars">("leetcode");

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

function openExternal(url: string): void {
  void window.kepler.shell.openExternal(url);
}
</script>

<template>
  <main class="coder-page kosmos-scroll">
    <div class="coder-content">
      <header class="coder-header">
        <div>
          <h1>Кодер</h1>
          <p>Статистика задач и отправок LeetCode.</p>
        </div>
        <Button variant="ghost" size="sm" :loading="loading || refreshing" @click="refresh">
          <template #icon><RefreshCw :size="13" /></template>
          Обновить
        </Button>
      </header>

      <nav class="platform-tabs" aria-label="Площадка">
        <button :aria-pressed="platform === 'leetcode'" @click="platform = 'leetcode'">
          LeetCode
        </button>
        <button :aria-pressed="platform === 'codewars'" @click="platform = 'codewars'">
          Codewars
        </button>
      </nav>

      <template v-if="platform === 'leetcode'">
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
              <span>Успешность</span
              ><strong>{{ Math.round(summary.acceptanceRate * 100) }}%</strong>
            </article>
            <article>
              <span>Серия</span><strong>{{ summary.currentStreak }} дн.</strong>
            </article>
            <article>
              <span>Лучшая серия</span><strong>{{ summary.longestStreak }} дн.</strong>
            </article>
          </section>

          <CoderDifficultyGauge :stats="difficulties" />
          <CoderActivity :days="activity" />
          <div class="breakdown-grid">
            <CoderBreakdown title="Языки" :items="languages" />
            <CoderBreakdown title="Результаты" :items="localizedStatuses" />
          </div>
          <CoderSubmissions :submissions="recent" @open="openExternal" />
        </template>
      </template>
      <EmptyState
        v-else
        title="Codewars пока не подключён"
        description="Подключение добавим следующим шагом."
        compact
      />
    </div>
  </main>
</template>

<style scoped>
.coder-page {
  --coder-accent: color-mix(in srgb, var(--foreground) 88%, var(--muted-foreground));
  --coder-divider: color-mix(in srgb, var(--foreground) 12%, transparent);
  --coder-easy: color-mix(in srgb, var(--status-success) 38%, var(--muted-foreground));
  --coder-medium: color-mix(in srgb, var(--status-warning) 40%, var(--muted-foreground));
  --coder-hard: color-mix(in srgb, var(--destructive) 36%, var(--muted-foreground));
  box-sizing: border-box;
  width: 100%;
  height: 100%;
  overflow-x: hidden;
  overflow-y: auto;
  scrollbar-gutter: stable;
  color: var(--foreground);
}

.coder-content {
  box-sizing: border-box;
  width: 100%;
  max-width: var(--kosmos-page-max-width, 700px);
  margin: 0 auto;
  padding: 20px 24px 28px;
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

.platform-tabs {
  display: flex;
  gap: 3px;
  margin-bottom: 18px;
  border-bottom: 1px solid var(--border);
}

.platform-tabs button {
  margin: 0 0 -1px;
  padding: 7px 10px;
  border: 0;
  border-bottom: 1px solid transparent;
  background: transparent;
  color: var(--muted-foreground);
  font: inherit;
  font-size: 0.75rem;
  cursor: pointer;
}

.platform-tabs button[aria-pressed="true"] {
  border-bottom-color: var(--coder-accent);
  color: var(--foreground);
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
  gap: 0;
  margin-bottom: 14px;
}

.summary-grid article {
  display: flex;
  align-items: center;
  min-width: 0;
  flex-direction: column;
  gap: 2px;
  border: 1px solid var(--coder-divider);
  border-radius: 0;
  padding: 13px 14px;
  background: transparent;
}

.summary-grid article + article {
  border-left: 0;
}

.summary-grid article:first-child {
  border-radius: 9px 0 0 9px;
}

.summary-grid article:last-child {
  border-radius: 0 9px 9px 0;
}

.summary-grid span {
  overflow: hidden;
  color: var(--muted-foreground);
  font-size: 0.6875rem;
  line-height: 1.25;
  text-align: center;
  text-overflow: ellipsis;
}

.summary-grid strong {
  order: -1;
  color: var(--foreground);
  font-size: 1.2rem;
}

.breakdown-grid {
  display: grid;
  grid-template-columns: repeat(2, minmax(0, 1fr));
  gap: 14px;
  margin: 14px 0;
}

@media (max-width: 640px) {
  .summary-grid,
  .breakdown-grid {
    grid-template-columns: 1fr;
  }

  .summary-grid article + article {
    border-top: 0;
    border-left: 1px solid var(--coder-divider);
  }

  .summary-grid article:first-child {
    border-radius: 9px 9px 0 0;
  }

  .summary-grid article:last-child {
    border-radius: 0 0 9px 9px;
  }
}
</style>
