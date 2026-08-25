<script setup lang="ts">
import type { CoderPlatform, CodingSubmission } from "./types";

defineProps<{ platform: CoderPlatform; submissions: CodingSubmission[] }>();
const emit = defineEmits<{ open: [url: string] }>();

const dateFormatter = new Intl.DateTimeFormat("ru", { dateStyle: "medium", timeStyle: "short" });

const statusLabels = {
  Accepted: "Принято",
  "Wrong Answer": "Неверный ответ",
  "Time Limit Exceeded": "Превышен лимит времени",
  "Memory Limit Exceeded": "Превышен лимит памяти",
  "Runtime Error": "Ошибка выполнения",
  "Compile Error": "Ошибка компиляции",
  "Output Limit Exceeded": "Превышен лимит вывода",
  "Internal Error": "Внутренняя ошибка",
  Unknown: "Неизвестно",
  Completed: "Завершено",
} satisfies Record<string, string>;

function statusLabel(status: string): string {
  // SAFETY: unknown upstream status labels intentionally fall back to themselves.
  return statusLabels[status as keyof typeof statusLabels] ?? status;
}

function problemNumber(value: string): string {
  return /^\d{1,4}$/.test(value) ? value.padStart(4, "0") : value || "—";
}

function leetcodeProblemUrl(slug: string): string {
  return slug ? `https://leetcode.com/problems/${encodeURIComponent(slug)}/` : "";
}
</script>

<template>
  <section class="submissions-panel">
    <header>
      <h2>{{ platform === "leetcode" ? "Последние отправки" : "Последние kata" }}</h2>
      <span>{{ submissions.length }}</span>
    </header>
    <div class="table-scroll kosmos-scroll">
      <table>
        <thead>
          <tr>
            <th v-if="platform === 'leetcode'" class="problem-number">№</th>
            <th>Задача</th>
            <th>Результат</th>
            <th>Язык</th>
            <th v-if="platform === 'leetcode'">Время</th>
            <th v-if="platform === 'leetcode'">Память</th>
            <th>Дата</th>
          </tr>
        </thead>
        <tbody>
          <tr v-for="submission in submissions" :key="submission.id">
            <td v-if="platform === 'leetcode'" class="problem-number">
              <a
                v-if="platform === 'leetcode' && submission.problemSlug"
                :href="leetcodeProblemUrl(submission.problemSlug)"
                @click.prevent="emit('open', leetcodeProblemUrl(submission.problemSlug))"
              >
                {{ problemNumber(submission.problemNumber) }}
              </a>
              <span v-else>{{ problemNumber(submission.problemNumber) }}</span>
            </td>
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
            <td>{{ submission.languages.join(", ") }}</td>
            <td v-if="platform === 'leetcode'">{{ submission.runtime || "—" }}</td>
            <td v-if="platform === 'leetcode'">{{ submission.memory || "—" }}</td>
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
  font-family: var(--font-mono, ui-monospace, monospace);
  font-size: 0.75rem;
  font-variant-numeric: tabular-nums;
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

.problem-number {
  width: 1%;
  padding-right: 8px;
  color: var(--muted-foreground);
  text-align: right;
}

tbody tr:last-child td {
  border-bottom: 0;
}

td button,
td a {
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

td a {
  text-decoration: none;
}

td button:not(:disabled):hover,
td a:hover {
  color: var(--coder-accent);
  text-decoration: underline;
}

.accepted {
  color: var(--coder-accent);
}
</style>
