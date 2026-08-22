<script setup lang="ts">
import { onMounted, ref } from "vue";
import { Button, TextInput } from "@kosmos/visuals";
import LegacyRow from "../components/LegacyRow.vue";
import LegacyToggle from "../components/LegacyToggle.vue";

type FileIndexSettings = {
  enabled: boolean;
  exclude_noisy_folders: boolean;
  roots: string[];
  ignore_patterns: string[];
  respect_gitignore: boolean;
  include_hidden: boolean;
  ntfs_accelerated: boolean;
  scan_in_progress: boolean;
  scan_progress: { message?: string };
  ntfs_status: "active" | "fallback" | "unavailable" | "disabled" | "unknown";
};

type FileIndexDiagnostics = {
  files_count: number;
  total_size_bytes: number;
  scan_in_progress: boolean;
};

type FileIndexSettingKey =
  | "enabled"
  | "exclude_noisy_folders"
  | "respect_gitignore"
  | "include_hidden"
  | "ntfs_accelerated";

const settings = ref<FileIndexSettings | null>(null);
const diagnostics = ref<FileIndexDiagnostics | null>(null);
const ignore = ref("");
const busy = ref(false);
const error = ref("");

function unwrap<T>(value: unknown): T {
  if (value && typeof value === "object" && "data" in value) {
    return (value as { data: T }).data;
  }
  return value as T;
}

async function request<T>(operation: string, params: Record<string, unknown> = {}): Promise<T> {
  return unwrap<T>(await window.kepler.ark.request(operation, params));
}

async function refresh() {
  try {
    error.value = "";
    const [nextSettings, nextDiagnostics] = await Promise.all([
      request<FileIndexSettings>("file_index.settings_get"),
      request<FileIndexDiagnostics>("file_index.diagnostics"),
    ]);
    settings.value = nextSettings;
    diagnostics.value = nextDiagnostics;
  } catch (cause) {
    error.value = cause instanceof Error ? cause.message : "Не удалось загрузить индекс файлов.";
  }
}

async function updateSetting(key: FileIndexSettingKey, value: boolean) {
  busy.value = true;
  try {
    await request("file_index.settings_set", { [key]: value });
    await refresh();
  } finally {
    busy.value = false;
  }
}

async function addRoot() {
  const path = await window.kepler.fileIndex.pickRoot();
  if (!path) return;
  await request("file_index.scope_add", { path });
  await refresh();
}

async function removeRoot(path: string) {
  await request("file_index.scope_remove", { path });
  await refresh();
}

async function addIgnore() {
  const pattern = ignore.value.trim();
  if (!pattern) return;
  await request("file_index.ignore_add", { pattern });
  ignore.value = "";
  await refresh();
}

async function removeIgnore(pattern: string) {
  await request("file_index.ignore_remove", { pattern });
  await refresh();
}

async function rescan() {
  await request("file_index.rescan");
  await refresh();
}

async function clearCache() {
  if (!window.confirm("Очистить перестраиваемый индекс файлов? Настройки и папки сохранятся."))
    return;
  await request("file_index.clear_cache");
  await refresh();
}

function formatBytes(bytes: number): string {
  if (bytes < 1024) return `${bytes} Б`;
  if (bytes < 1024 * 1024) return `${Math.round(bytes / 1024)} КБ`;
  return `${(bytes / (1024 * 1024)).toFixed(1)} МБ`;
}

onMounted(() => void refresh());
</script>

<template>
  <div class="rows shell-file-index">
    <p v-if="error" class="error" role="alert">{{ error }}</p>

    <section class="settings-card">
      <h2>Индекс файлов</h2>
      <p class="hint">Shell использует результаты Engine для поиска файлов и приложений.</p>
      <LegacyRow title="Индексировать файлы" hint="Включить локальное сканирование">
        <LegacyToggle
          :checked="settings?.enabled ?? false"
          :disabled="busy || !settings"
          @change="(event) => updateSetting('enabled', (event.target as HTMLInputElement).checked)"
        />
      </LegacyRow>
      <LegacyRow title="Исключать шумные папки">
        <LegacyToggle
          :checked="settings?.exclude_noisy_folders ?? false"
          :disabled="busy || !settings"
          @change="
            (event) =>
              updateSetting('exclude_noisy_folders', (event.target as HTMLInputElement).checked)
          "
        />
      </LegacyRow>
      <LegacyRow title="Учитывать .gitignore">
        <LegacyToggle
          :checked="settings?.respect_gitignore ?? false"
          :disabled="busy || !settings"
          @change="
            (event) =>
              updateSetting('respect_gitignore', (event.target as HTMLInputElement).checked)
          "
        />
      </LegacyRow>
      <LegacyRow title="Включать скрытые файлы">
        <LegacyToggle
          :checked="settings?.include_hidden ?? false"
          :disabled="busy || !settings"
          @change="
            (event) => updateSetting('include_hidden', (event.target as HTMLInputElement).checked)
          "
        />
      </LegacyRow>
      <LegacyRow title="Ускорять через NTFS">
        <LegacyToggle
          :checked="settings?.ntfs_accelerated ?? false"
          :disabled="busy || !settings"
          @change="
            (event) => updateSetting('ntfs_accelerated', (event.target as HTMLInputElement).checked)
          "
        />
      </LegacyRow>
    </section>

    <section class="settings-card">
      <div class="section-heading">
        <h2>Папки индексации</h2>
        <Button size="sm" @click="addRoot">Добавить папку</Button>
      </div>
      <div v-if="settings?.roots.length" class="settings-list">
        <div v-for="root in settings.roots" :key="root" class="settings-list-row">
          <span>{{ root }}</span>
          <Button variant="ghost" size="sm" @click="removeRoot(root)">Удалить</Button>
        </div>
      </div>
      <p v-else class="hint">Папки пока не выбраны.</p>
    </section>

    <section class="settings-card">
      <h2>Исключения</h2>
      <div class="ignore-form">
        <TextInput
          v-model="ignore"
          maxlength="512"
          placeholder="Например, node_modules"
          @keyup.enter="addIgnore"
        />
        <Button size="sm" :disabled="!ignore.trim()" @click="addIgnore">Добавить</Button>
      </div>
      <div v-if="settings?.ignore_patterns.length" class="settings-list">
        <div v-for="pattern in settings.ignore_patterns" :key="pattern" class="settings-list-row">
          <span>{{ pattern }}</span>
          <Button variant="ghost" size="sm" @click="removeIgnore(pattern)">Удалить</Button>
        </div>
      </div>
    </section>

    <div class="index-stats">
      <div class="settings-card">
        <span>Файлов в индексе</span><strong>{{ diagnostics?.files_count ?? 0 }}</strong>
      </div>
      <div class="settings-card">
        <span>Размер индекса</span
        ><strong>{{ formatBytes(diagnostics?.total_size_bytes ?? 0) }}</strong>
      </div>
      <div class="settings-card">
        <span>NTFS</span><strong>{{ settings?.ntfs_status ?? "неизвестно" }}</strong>
      </div>
    </div>

    <section class="settings-card">
      <p class="hint">{{ settings?.scan_progress.message || "Состояние индекса не загружено." }}</p>
      <p v-if="settings?.scan_in_progress || diagnostics?.scan_in_progress">
        Сканирование выполняется…
      </p>
      <div class="actions">
        <Button variant="ghost" :disabled="!settings" @click="rescan">Пересканировать</Button>
        <Button variant="ghost" :disabled="!settings" @click="clearCache">Очистить кэш</Button>
        <Button variant="ghost" @click="refresh">Обновить</Button>
      </div>
    </section>
  </div>
</template>

<style scoped>
.shell-file-index {
  gap: 12px;
}
.settings-card {
  padding: 16px;
  border: 1px solid var(--border-subtle);
  border-radius: 12px;
  background: var(--settings-list-background, var(--background));
}
.settings-card h2 {
  margin: 0 0 8px;
  font-size: 16px;
}
.hint {
  color: var(--muted-foreground);
}
.section-heading,
.settings-list-row,
.ignore-form,
.actions {
  display: flex;
  align-items: center;
  gap: 8px;
}
.section-heading,
.settings-list-row {
  justify-content: space-between;
}
.settings-list {
  display: grid;
  gap: 6px;
  margin-top: 10px;
}
.settings-list-row {
  padding: 8px 0;
  border-top: 1px solid var(--border-subtle);
}
.ignore-form :deep(input) {
  flex: 1;
}
.index-stats {
  display: grid;
  grid-template-columns: repeat(3, minmax(0, 1fr));
  gap: 12px;
}
.index-stats strong {
  display: block;
  margin-top: 8px;
  font-size: 20px;
}
</style>
