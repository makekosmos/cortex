<script setup lang="ts">
import type { CodingSubmission } from "./types";

defineProps<{ submissions: CodingSubmission[] }>();
const emit = defineEmits<{ open: [url: string] }>();

const dateFormatter = new Intl.DateTimeFormat("ru", { dateStyle: "medium", timeStyle: "short" });

const statusLabels: Record<string, string> = {
  Accepted: "Принято",
  "Wrong Answer": "Неверный ответ",
  "Time Limit Exceeded": "Превышен лимит времени",
  "Memory Limit Exceeded": "Превышен лимит памяти",
  "Runtime Error": "Ошибка выполнения",
  "Compile Error": "Ошибка компиляции",
  "Output Limit Exceeded": "Превышен лимит вывода",
  "Internal Error": "Внутренняя ошибка",
  Unknown: "Неизвестно",
};

function statusLabel(status: string): string {
  return statusLabels[status] ?? status;
}
</script>

<template>
  <section class="submissions-panel">
    <header>
      <h2>Последние отправки</h2>
      <span>{{ submissions.length }}</span>
    </header>
    <div class="table-scroll kosmos-scroll">
      <table>
        <thead>
          <tr>
            <th>Задача</th>
            <th>Результат</th>
            <th>Язык</th>
            <th>Время</th>
            <th>Память</th>
            <th>Дата</th>
          </tr>
        </thead>
        <tbody>
          <tr v-for="submission in submissions" :key="submission.id">
            <td>
              <button
                type="button"
                :disabled="!submission.url"
                @click="submission.url && emit('open', submission.url)"
              >
                {{ submission.problemTitle }}
              </button>
            </td>
            <td :class="{ accepted: submission.accepted }">{{ statusLabel(submission.status) }}</td>
            <td>{{ submission.language }}</td>
            <td>{{ submission.runtime || "—" }}</td>
            <td>{{ submission.memory || "—" }}</td>
            <td>{{ dateFormatter.format(new Date(submission.submittedAt)) }}</td>
          </tr>
        </tbody>
      </table>
    </div>
  </section>
</template>

<style scoped>
.submissions-panel {
  overflow: hidden;
  border: 1px solid var(--border);
  border-radius: 10px;
  background: var(--settings-list-background, var(--background));
}

header {
  display: flex;
  align-items: center;
  justify-content: space-between;
  padding: 14px 16px;
  border-bottom: 1px solid var(--border);
}

h2 {
  margin: 0;
  color: var(--foreground);
  font-size: 0.875rem;
}

header span {
  color: var(--muted-foreground);
  font-size: 0.6875rem;
}

.table-scroll {
  overflow-x: auto;
}

table {
  width: 100%;
  border-collapse: collapse;
  color: var(--foreground);
  font-size: 0.75rem;
  white-space: nowrap;
}

th,
td {
  padding: 10px 14px;
  border-bottom: 1px solid var(--border);
  text-align: left;
}

th {
  color: var(--muted-foreground);
  font-size: 0.6875rem;
  font-weight: 500;
}

tbody tr:last-child td {
  border-bottom: 0;
}

td button {
  max-width: 260px;
  overflow: hidden;
  border: 0;
  padding: 0;
  background: transparent;
  color: var(--foreground);
  font: inherit;
  text-overflow: ellipsis;
  white-space: nowrap;
}

td button:not(:disabled):hover {
  color: var(--accent);
  text-decoration: underline;
}

.accepted {
  color: var(--accent);
}
</style>
