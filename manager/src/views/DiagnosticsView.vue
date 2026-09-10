<script setup lang="ts">
import { computed, onMounted, ref } from "vue";
import { Button, SettingsList, SettingsRow, StatusDot } from "@kosmos/visuals";
import type {
  CrashReportMetadata,
  DiagnosticItem,
  DiagnosticsSnapshot,
  JsonRecord,
} from "../manager-api";
import type { ManagerClient } from "../composables/useManagerClient";

const props = defineProps<{ client: ManagerClient }>();
const snapshot = ref<DiagnosticsSnapshot | null>(null);
const tail = ref<JsonRecord[]>([]);
const crashes = ref<CrashReportMetadata[]>([]);
const components = computed<DiagnosticItem[]>(() => snapshot.value?.components ?? []);
const workers = computed<DiagnosticItem[]>(() => snapshot.value?.workers ?? []);
const gate = computed(() => snapshot.value?.legacy_gate);
function componentLabel(name?: string): string {
  if (name === "rpc") return "API движка";
  if (name === "usage_tracker") return "Учёт активности";
  if (name === "protocol_usage") return "Протокол подключений";
  return name || "Компонент";
}
function statusLabel(status?: string): string {
  if (status === "running" || status === "healthy" || status === "ready" || status === "ok")
    return "работает";
  if (status === "stopped" || status === "disabled") return "остановлен";
  if (status === "restarting") return "перезапускается";
  if (status === "failed" || status === "error") return "ошибка";
  return "состояние неизвестно";
}

async function refresh() {
  snapshot.value = await props.client.call<DiagnosticsSnapshot>(
    "getDiagnosticsSnapshot",
    undefined,
    "diagnostics",
  );
  const result = await props.client.call<{ entries: JsonRecord[] }>(
    "getDiagnosticLogTail",
    { lines: 100 },
    "log-tail",
  );
  tail.value = result?.entries ?? [];
  crashes.value =
    (await props.client.call<CrashReportMetadata[]>("listCrashReports", undefined, "crashes")) ??
    [];
}
async function saveBundle() {
  await props.client.call("saveSupportBundle", undefined, "support-bundle");
}
async function clearCrashes() {
  await props.client.call("clearCrashReports", undefined, "crashes");
  await refresh();
}
async function openCrashes() {
  await props.client.call("openCrashReportsFolder", undefined, "open-crashes");
}
async function openLogs() {
  await props.client.call("openLogsFolder", undefined, "open-logs");
}
function logText(entry: JsonRecord): string {
  const value = entry.message ?? entry.line ?? entry.operation;
  return value == null ? "Диагностическая запись" : String(value);
}
onMounted(() => void refresh());
</script>

<template>
  <section class="stack">
    <div class="toolbar">
      <Button variant="ghost" size="sm" @click="refresh">Обновить диагностику</Button>
    </div>
    <div class="cards">
      <article v-for="item in components" :key="String(item.name)" class="card">
        <div class="card-heading">
          <span>{{ componentLabel(item.name) }}</span
          ><StatusDot
            :tone="
              item.status === 'healthy' || item.status === 'running' || item.state === 'ok'
                ? 'success'
                : 'warning'
            "
          />
        </div>
        <strong>{{ statusLabel(item.status ?? item.state) }}</strong
        ><small>{{ item.message ?? "Диагностика доступна" }}</small>
      </article>
    </div>
    <div class="list-section">
      <h2 class="list-title">Рабочие процессы</h2>
      <SettingsList>
        <SettingsRow
          v-for="worker in workers"
          :key="String(worker.id ?? worker.name)"
          :title="String(worker.name ?? worker.id)"
          description="Рабочий процесс"
        >
          <template #control
            ><small>{{ statusLabel(worker.status ?? worker.state) }}</small></template
          >
        </SettingsRow>
        <SettingsRow v-if="!workers.length" title="Состояния рабочих процессов не сообщены" />
      </SettingsList>
    </div>
    <div class="card">
      <div class="card-heading">
        <h2>Отчёты об ошибках</h2>
        <strong>{{ crashes.length }}</strong>
      </div>
      <p v-for="crash in crashes" :key="crash.name" class="row">
        <span>{{ crash.name }}</span
        ><small>{{ crash.size }} Б · {{ crash.mtime }}</small>
      </p>
      <p v-if="!crashes.length" class="muted">Отчётов нет.</p>
      <div class="toolbar">
        <Button variant="ghost" size="sm" @click="openCrashes">Открыть папку</Button>
        <Button variant="ghost" size="sm" :disabled="!crashes.length" @click="clearCrashes"
          >Очистить</Button
        >
        <Button variant="ghost" size="sm" @click="openLogs">Открыть логи</Button>
      </div>
    </div>
    <div class="card">
      <h2>Удаление старого протокола</h2>
      <p class="muted">
        Подключений API v1: {{ gate?.api_v1_connections ?? 0 }}. Старых подключений:
        {{ gate?.legacy_connections ?? 0 }}. Отслеживание начато:
        {{ gate?.tracking_started_at ?? "дата недоступна" }}.
      </p>
      <p class="muted">
        Последнее старое подключение:
        {{ gate?.legacy_last_seen ?? "не зафиксировано" }}. Условие 30 дней без старого протокола:
        {{ gate?.ready ? "выполнено" : "не выполнено" }}.
      </p>
    </div>
    <div class="list">
      <h2 class="list-title">Последние сообщения</h2>
      <p v-for="(line, index) in tail" :key="index" class="log-line">
        {{ logText(line) }}
      </p>
      <p v-if="!tail.length" class="muted">Сообщений нет.</p>
    </div>
    <div class="card">
      <h2>Пакет поддержки</h2>
      <p class="muted">
        Будет создан локальный предварительно обезличенный файл. Удалённой отправки нет.
      </p>
      <Button @click="saveBundle">Сохранить пакет поддержки</Button>
    </div>
  </section>
</template>
