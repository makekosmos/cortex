<script setup lang="ts">
import { computed, onMounted, ref } from "vue";
import { Button, SettingsList, SettingsRow, SettingsToggleRow } from "@kosmos/visuals";
import type { DatabaseBackup, DbBackupRestoreResult, DbBackupValidation } from "../manager-api";
import type { ManagerClient } from "../composables/useManagerClient";

const props = defineProps<{ client: ManagerClient }>();
const enabled = ref(false);
const available = ref(false);
const trayIcon = ref(true);
const backups = ref<DatabaseBackup[]>([]);
const backupBusy = ref(false);
const backupMessage = ref<string | null>(null);
const label = computed(() => (enabled.value ? "Включён" : "Отключён"));
const lastBackup = computed(() => backups.value[0]?.modified_at ?? "нет снимков");

type TraySettingsBridge = {
  trayIcon: { get(): Promise<boolean>; set(enabled: boolean): Promise<void> };
};

function trayBridge() {
  // SAFETY: preload exposes the optional kepler settings bridge on window.
  return (window as typeof window & { kepler?: { settings?: TraySettingsBridge } }).kepler
    ?.settings;
}

async function load() {
  const result = await props.client.call<{
    enabled: boolean;
    available: boolean;
  }>("getAutostart", undefined, "autostart");
  if (!result) return;
  enabled.value = result.enabled;
  available.value = result.available;
  try {
    const bridge = trayBridge();
    trayIcon.value = bridge
      ? await bridge.trayIcon.get()
      : localStorage.getItem("kosmos.trayIcon") !== "false";
  } catch {
    trayIcon.value = true;
  }
}

async function setAutostart(value: boolean) {
  const result = await props.client.call<{
    enabled: boolean;
    available: boolean;
  }>("setAutostart", { enabled: value }, "autostart");
  if (!result) return;
  enabled.value = result.enabled;
  available.value = result.available;
}

async function setTrayIcon(value: boolean) {
  trayIcon.value = value;
  const bridge = trayBridge();
  try {
    if (bridge) await bridge.trayIcon.set(value);
    else localStorage.setItem("kosmos.trayIcon", String(value));
  } catch {
    localStorage.setItem("kosmos.trayIcon", String(value));
  }
}

async function downloadLogs() {
  await props.client.call("saveSupportBundle", undefined, "support-bundle");
}

function formatBytes(size: number): string {
  if (size >= 1024 * 1024) return `${(size / 1024 / 1024).toFixed(1)} МБ`;
  if (size >= 1024) return `${(size / 1024).toFixed(1)} КБ`;
  return `${size} Б`;
}

function validationReason(check: DbBackupValidation): string {
  if (check.error) return check.error;
  if (!check.exists) return "файл не найден в папке снимков";
  if (!check.integrity_ok) return "файл повреждён";
  if (!check.schema_match) return "несовместимая схема данных";
  return "неизвестная причина";
}

async function validateDbBackup(backup: DatabaseBackup): Promise<DbBackupValidation | null> {
  const check = await props.client.call<DbBackupValidation>(
    "validateDbBackup",
    { backup_id: backup.name },
    `db-backup-validate-${backup.name}`,
  );
  return check;
}

async function checkBackup(backup: DatabaseBackup) {
  if (backupBusy.value) return;
  backupBusy.value = true;
  backupMessage.value = null;
  try {
    const check = await validateDbBackup(backup);
    if (!check) return;
    backupMessage.value = check.valid
      ? `Снимок ${backup.name} годен к восстановлению.`
      : `Снимок ${backup.name} не годен к восстановлению: ${validationReason(check)}`;
  } finally {
    backupBusy.value = false;
  }
}

async function restoreBackup(backup: DatabaseBackup) {
  if (backupBusy.value) return;
  if (
    !window.confirm(
      `Восстановить базу данных из снимка ${backup.name}?\n\nТекущие данные будут заменены содержимым снимка. Открытые окна Kosmos перезагрузятся.`,
    )
  )
    return;
  backupBusy.value = true;
  backupMessage.value = null;
  try {
    const check = await validateDbBackup(backup);
    if (!check) return;
    if (!check.valid) {
      backupMessage.value = `Восстановление отклонено — снимок не годен: ${validationReason(check)}`;
      return;
    }
    const restored = await props.client.call<DbBackupRestoreResult>(
      "restoreDbBackup",
      { backup_id: backup.name },
      "db-backup-restore",
    );
    if (!restored) return;
    backupMessage.value = `Восстановлено: ${restored.objects} объектов, ${restored.links} связей. Manager перезагружается…`;
    window.setTimeout(() => window.location.reload(), 1500);
  } finally {
    backupBusy.value = false;
  }
}

async function loadBackups(): Promise<boolean> {
  const result = await props.client.call<DatabaseBackup[]>("getDbBackups", undefined, "db-backups");
  if (!result) return false;
  backups.value = result;
  return true;
}

async function createBackup() {
  backupBusy.value = true;
  backupMessage.value = null;
  try {
    const result = await props.client.call<{ path: string }>(
      "createDbBackup",
      undefined,
      "db-backup-create",
    );
    if (!result) return;
    backupMessage.value = (await loadBackups())
      ? "Снимок базы создан."
      : "Снимок создан, но список обновить не удалось.";
  } finally {
    backupBusy.value = false;
  }
}

async function openBackupsFolder() {
  await props.client.call("openDbBackupsFolder", undefined, "db-backups-open");
}

onMounted(() => {
  void load();
  void loadBackups();
});
</script>

<template>
  <section class="stack settings-page">
    <SettingsList>
      <SettingsToggleRow
        title="Запускать Kosmos при входе в систему"
        description="Автоматически запускать Kosmos после входа в Windows."
        :model-value="enabled"
        :label="label"
        :disabled="!available"
        @update:model-value="setAutostart"
      />
      <SettingsToggleRow
        title="Показывать Kosmos в системном трее"
        description="Управляет значком Kosmos в области уведомлений Windows."
        :model-value="trayIcon"
        :label="trayIcon ? 'Показывать' : 'Скрывать'"
        @update:model-value="setTrayIcon"
      />
      <SettingsRow
        title="Скачать логи"
        description="Сохранить локальный архив с журналами приложения."
      >
        <template #control>
          <Button size="sm" variant="surface" @click="downloadLogs">Скачать</Button>
        </template>
      </SettingsRow>
    </SettingsList>
    <div class="card">
      <div class="card-heading">
        <h2>Резервные копии базы</h2>
        <small>Последний снимок: {{ lastBackup }}</small>
      </div>
      <p class="muted">
        Это локальные снимки базы данных ARK, а не синхронизация, Huawei, Store или экспорт.
      </p>
      <p class="muted">
        Восстановление заменяет текущие данные содержимым снимка; перед ним снимок проверяется, а
        открытые окна Kosmos перезагружаются.
      </p>
      <div class="toolbar">
        <Button :disabled="backupBusy" @click="createBackup">Сделать бэкап сейчас</Button>
        <Button variant="ghost" :disabled="backupBusy" @click="openBackupsFolder"
          >Открыть папку бэкапов</Button
        >
      </div>
      <p v-if="backupMessage" class="muted" role="status">{{ backupMessage }}</p>
      <SettingsList v-if="backups.length">
        <SettingsRow
          v-for="backup in backups"
          :key="backup.name"
          :title="backup.name"
          :description="`${formatBytes(backup.size)} · ${backup.modified_at ?? 'нет даты'}`"
        >
          <template #control>
            <Button size="sm" variant="ghost" :disabled="backupBusy" @click="checkBackup(backup)"
              >Проверить</Button
            >
            <Button size="sm" :disabled="backupBusy" @click="restoreBackup(backup)"
              >Восстановить</Button
            >
          </template>
        </SettingsRow>
      </SettingsList>
      <p v-else class="muted">Снимков пока нет.</p>
    </div>
  </section>
</template>
