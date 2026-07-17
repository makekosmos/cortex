<script setup lang="ts">
import { computed, onMounted, ref } from "vue";
import { Check, ExternalLink, LogIn, RefreshCw, Trash2 } from "@lucide/vue";
import { Button, Toggle, useToast } from "@kosmos/visuals";

type ProviderId = "hevy" | "toggl" | "leetcode";

const props = withDefaults(
  defineProps<{
    providerId?: ProviderId;
    compact?: boolean;
  }>(),
  {
    providerId: undefined,
    compact: false,
  },
);

interface ProviderSettings {
  intervalMinutes: number;
  syncOnStartup: boolean;
  lastAttemptAt: string | null;
  lastSuccessAt: string | null;
  lastError: string | null;
  importedCount: number;
}

interface ProviderSnapshot {
  id: ProviderId;
  label: string;
  credentialLabel: string;
  credentialUrl: string;
  hasCredential: boolean;
  settings: ProviderSettings;
}

interface IntegrationsSnapshot {
  providers: ProviderSnapshot[];
  bodyWeightKg: number | null;
}

const intervals = [
  { value: 0, label: "Только вручную" },
  { value: 15, label: "Каждые 15 минут" },
  { value: 60, label: "Каждый час" },
  { value: 360, label: "Каждые 6 часов" },
  { value: 1440, label: "Раз в сутки" },
];

const toast = useToast();
const snapshot = ref<IntegrationsSnapshot | null>(null);
const loading = ref(true);
const error = ref("");
const credentials = ref<Record<ProviderId, string>>({ hevy: "", toggl: "", leetcode: "" });
const busy = ref(new Set<string>());
const providers = computed(() =>
  (snapshot.value?.providers ?? []).filter(
    (provider) => !props.providerId || provider.id === props.providerId,
  ),
);

function setBusy(key: string, value: boolean) {
  const next = new Set(busy.value);
  if (value) next.add(key);
  else next.delete(key);
  busy.value = next;
}

function isBusy(provider: ProviderId, action: string) {
  return busy.value.has(`${provider}:${action}`);
}

async function request<T>(operation: string, params: Record<string, unknown> = {}) {
  return (await window.kepler.ark.request(operation, params)) as T;
}

async function load() {
  loading.value = true;
  error.value = "";
  try {
    snapshot.value = await request<IntegrationsSnapshot>("integrations.list");
  } catch (cause) {
    error.value = cause instanceof Error ? cause.message : "Не удалось загрузить интеграции";
  } finally {
    loading.value = false;
  }
}

async function updateSettings(
  provider: ProviderSnapshot,
  patch: Partial<Pick<ProviderSettings, "intervalMinutes" | "syncOnStartup">>,
) {
  setBusy(provider.id, "settings", true);
  try {
    snapshot.value = await request<IntegrationsSnapshot>("integrations.update_settings", {
      provider: provider.id,
      ...patch,
    });
  } catch (cause) {
    toast.error(cause instanceof Error ? cause.message : "Не удалось сохранить настройки");
  } finally {
    setBusy(provider.id, "settings", false);
  }
}

async function saveCredential(provider: ProviderSnapshot) {
  const credential = credentials.value[provider.id].trim();
  if (!credential) return;
  setBusy(provider.id, "credential", true);
  try {
    snapshot.value = await request<IntegrationsSnapshot>("integrations.set_credential", {
      provider: provider.id,
      credential,
    });
    credentials.value[provider.id] = "";
    toast.success(`${provider.label}: ключ проверен и сохранён`);
  } catch (cause) {
    toast.error(cause instanceof Error ? cause.message : "Ключ не прошёл проверку");
  } finally {
    setBusy(provider.id, "credential", false);
  }
}

async function clearCredential(provider: ProviderSnapshot) {
  setBusy(provider.id, "credential", true);
  try {
    snapshot.value =
      provider.id === "leetcode"
        ? await window.kepler.integrations.disconnectLeetCode<IntegrationsSnapshot>()
        : await request<IntegrationsSnapshot>("integrations.clear_credential", {
            provider: provider.id,
          });
    credentials.value[provider.id] = "";
    toast.success(`${provider.label}: ключ удалён`);
  } catch (cause) {
    toast.error(cause instanceof Error ? cause.message : "Не удалось удалить ключ");
  } finally {
    setBusy(provider.id, "credential", false);
  }
}

async function connectLeetCode(provider: ProviderSnapshot) {
  setBusy(provider.id, "credential", true);
  try {
    snapshot.value = await window.kepler.integrations.connectLeetCode<IntegrationsSnapshot>();
    toast.success("LeetCode подключён");
  } catch (cause) {
    toast.error(cause instanceof Error ? cause.message : "Не удалось войти в LeetCode");
  } finally {
    setBusy(provider.id, "credential", false);
  }
}

async function syncNow(provider: ProviderSnapshot) {
  setBusy(provider.id, "sync", true);
  try {
    const result = await request<{ imported: number }>("integrations.sync_now", {
      provider: provider.id,
    });
    await load();
    toast.success(`${provider.label}: импортировано ${result.imported}`);
  } catch (cause) {
    await load();
    toast.error(cause instanceof Error ? cause.message : "Синхронизация не удалась");
  } finally {
    setBusy(provider.id, "sync", false);
  }
}

function formatDate(value: string | null) {
  if (!value) return "Ещё не запускалась";
  return new Intl.DateTimeFormat("ru", { dateStyle: "medium", timeStyle: "short" }).format(
    new Date(value),
  );
}

function openCredentialPage(provider: ProviderSnapshot) {
  void window.kepler.shell.openExternal(provider.credentialUrl);
}

onMounted(load);
</script>

<template>
  <div class="integrations-page kosmos-scroll" :class="{ 'integrations-page--compact': compact }">
    <p v-if="!compact" class="integrations-intro">
      Данные импортируются в общую базу Kosmos. Ключи остаются в защищённом хранилище этого
      устройства. Для LeetCode сохраняется только сессия входа, код решений не загружается.
    </p>

    <div v-if="loading" class="integrations-message">Загрузка…</div>
    <div v-else-if="error" class="integrations-message integrations-message--error">
      {{ error }}
    </div>

    <article v-for="provider in providers" v-else :key="provider.id" class="provider-card">
      <header class="provider-header">
        <div>
          <h2>{{ provider.label }}</h2>
          <p>
            {{
              provider.id === "hevy"
                ? "Тренировки и упражнения"
                : provider.id === "toggl"
                  ? "Записи учёта времени"
                  : "Задачи и история отправок"
            }}
          </p>
        </div>
        <span :class="['provider-status', { 'provider-status--ready': provider.hasCredential }]">
          <Check v-if="provider.hasCredential" :size="13" />
          {{
            provider.hasCredential
              ? "Подключено"
              : provider.id === "leetcode"
                ? "Нужен вход"
                : "Нужен ключ"
          }}
        </span>
      </header>

      <div v-if="provider.id === 'leetcode'" class="provider-section">
        <div class="credential-row credential-row--login">
          <p>
            Откроется отдельное окно LeetCode. Войдите обычным способом — пароль Kosmos не увидит.
          </p>
          <Button
            size="sm"
            :loading="isBusy(provider.id, 'credential')"
            @click="connectLeetCode(provider)"
          >
            <template #icon><LogIn :size="13" /></template>
            {{ provider.hasCredential ? "Войти заново" : "Войти через LeetCode" }}
          </Button>
          <Button
            v-if="provider.hasCredential"
            variant="danger"
            size="sm"
            :disabled="isBusy(provider.id, 'credential')"
            @click="clearCredential(provider)"
          >
            <template #icon><Trash2 :size="13" /></template>
            Отключить
          </Button>
        </div>
      </div>

      <div v-else class="provider-section">
        <div class="provider-section__heading">
          <span>{{ provider.credentialLabel }}</span>
          <button type="button" class="provider-link" @click="openCredentialPage(provider)">
            Где получить <ExternalLink :size="13" />
          </button>
        </div>
        <div class="credential-row">
          <input
            v-model="credentials[provider.id]"
            type="password"
            autocomplete="off"
            :placeholder="
              provider.hasCredential ? 'Введите новый ключ для замены' : 'Вставьте ключ'
            "
            :aria-label="`${provider.credentialLabel} для ${provider.label}`"
            @keydown.enter="saveCredential(provider)"
          />
          <Button
            size="sm"
            :disabled="!credentials[provider.id].trim()"
            :loading="isBusy(provider.id, 'credential')"
            @click="saveCredential(provider)"
          >
            {{ provider.hasCredential ? "Заменить" : "Подключить" }}
          </Button>
          <Button
            v-if="provider.hasCredential"
            variant="danger"
            size="sm"
            aria-label="Удалить ключ"
            :disabled="isBusy(provider.id, 'credential')"
            @click="clearCredential(provider)"
          >
            <template #icon><Trash2 :size="13" /></template>
            Удалить
          </Button>
        </div>
      </div>

      <div class="provider-grid">
        <label class="provider-field">
          <span>Частота получения данных</span>
          <select
            :value="provider.settings.intervalMinutes"
            :disabled="isBusy(provider.id, 'settings')"
            @change="
              updateSettings(provider, {
                intervalMinutes: Number(($event.target as HTMLSelectElement).value),
              })
            "
          >
            <option v-for="option in intervals" :key="option.value" :value="option.value">
              {{ option.label }}
            </option>
          </select>
        </label>
      </div>
      <p class="provider-sync-note">
        {{
          provider.id === "leetcode"
            ? "При первом импорте загружается вся доступная история отправок. Затем — данные с последней успешной синхронизации с перекрытием в сутки. Записи обновляются по ID без дублей; код решений не запрашивается. Первый импорт может занять несколько минут."
            : "При первом импорте загружается вся доступная история. Затем — только изменения после последней успешной синхронизации. Записи сопоставляются по ID сервиса без создания дублей; изменённые в сервисе записи обновляют импортированные поля в Kosmos."
        }}
      </p>

      <div class="startup-row">
        <div>
          <strong>Получать данные при входе в систему</strong>
          <span>Запрос выполнится при запуске Kosmos.</span>
        </div>
        <Toggle
          :model-value="provider.settings.syncOnStartup"
          :disabled="isBusy(provider.id, 'settings')"
          :aria-label="`Получать данные ${provider.label} при входе в систему`"
          @update:model-value="updateSettings(provider, { syncOnStartup: $event })"
        />
      </div>

      <footer class="provider-footer">
        <div class="provider-last-sync">
          <span>Последняя синхронизация: {{ formatDate(provider.settings.lastSuccessAt) }}</span>
          <span v-if="provider.settings.lastError" class="provider-error">{{
            provider.settings.lastError
          }}</span>
        </div>
        <Button
          variant="ghost"
          size="sm"
          :loading="isBusy(provider.id, 'sync')"
          :disabled="!provider.hasCredential"
          @click="syncNow(provider)"
        >
          <template #icon><RefreshCw :size="13" /></template>
          Получить сейчас
        </Button>
      </footer>
    </article>
  </div>
</template>

<style scoped>
.integrations-page {
  display: flex;
  flex: 1;
  min-height: 0;
  flex-direction: column;
  gap: 14px;
  overflow-y: auto;
  padding: 16px;
}

.integrations-page--compact {
  padding: 0;
}

.integrations-page--compact .provider-card {
  border: 0;
  background: transparent;
}

.integrations-intro,
.integrations-message {
  margin: 0;
  color: var(--muted-foreground);
  font-size: var(--kosmos-text-caption-size);
  line-height: 1.5;
}

.integrations-message--error,
.provider-error {
  color: var(--destructive);
}

.provider-card {
  overflow: hidden;
  border: 1px solid var(--border);
  border-radius: 12px;
  background: var(--settings-list-background, var(--background));
}

.provider-header,
.provider-footer,
.startup-row,
.provider-section,
.provider-grid {
  padding: 14px 16px;
}

.provider-header,
.provider-footer,
.startup-row,
.provider-section__heading,
.credential-row {
  display: flex;
  align-items: center;
  gap: 10px;
}

.provider-header,
.provider-footer,
.startup-row,
.provider-section__heading {
  justify-content: space-between;
}

.provider-header h2 {
  margin: 0;
  color: var(--foreground);
  font-size: 0.9375rem;
}

.provider-header p,
.startup-row span,
.provider-last-sync {
  margin: 3px 0 0;
  color: var(--muted-foreground);
  font-size: 0.6875rem;
}

.provider-status {
  display: inline-flex;
  align-items: center;
  gap: 4px;
  border-radius: 999px;
  padding: 4px 8px;
  background: color-mix(in srgb, var(--foreground) 7%, transparent);
  color: var(--muted-foreground);
  font-size: 0.6875rem;
}

.provider-status--ready {
  background: color-mix(in srgb, var(--accent) 14%, transparent);
  color: var(--accent);
}

.provider-section,
.provider-grid,
.provider-sync-note,
.startup-row,
.provider-footer {
  border-top: 1px solid var(--border);
}

.provider-sync-note {
  margin: 0;
  padding: 10px 16px;
  color: var(--muted-foreground);
  font-size: 0.6875rem;
  line-height: 1.45;
}

.provider-section__heading,
.provider-field > span:first-child,
.startup-row strong {
  color: var(--foreground);
  font-size: 0.75rem;
  font-weight: 600;
}

.provider-link {
  display: inline-flex;
  align-items: center;
  gap: 4px;
  border: 0;
  background: transparent;
  color: var(--accent);
  font: inherit;
  font-size: 0.6875rem;
}

.credential-row {
  margin-top: 10px;
}

.credential-row--login {
  margin-top: 0;
}

.credential-row--login p {
  min-width: 0;
  flex: 1;
  margin: 0;
  color: var(--muted-foreground);
  font-size: 0.6875rem;
  line-height: 1.45;
}

.credential-row > input,
.provider-field select {
  min-width: 0;
  border: 1px solid var(--border);
  border-radius: 7px;
  outline: none;
  background: var(--background);
  color: var(--foreground);
  font: inherit;
  font-size: 0.75rem;
}

.credential-row > input {
  flex: 1;
  height: 30px;
  padding: 0 10px;
}

.credential-row > input:focus,
.provider-field select:focus {
  border-color: var(--accent);
}

.provider-grid {
  display: grid;
  grid-template-columns: minmax(0, 1fr);
  gap: 16px;
}

.provider-field {
  display: flex;
  flex-direction: column;
  gap: 7px;
}

.provider-field select {
  height: 30px;
  padding: 0 8px;
}

.startup-row > div {
  display: flex;
  flex-direction: column;
}

.provider-last-sync {
  display: flex;
  min-width: 0;
  flex: 1;
  flex-direction: column;
  gap: 3px;
}

@media (max-width: 680px) {
  .provider-grid {
    grid-template-columns: 1fr;
  }

  .credential-row {
    align-items: stretch;
    flex-direction: column;
  }
}
</style>
