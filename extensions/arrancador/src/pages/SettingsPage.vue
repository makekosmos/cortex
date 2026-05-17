<script setup lang="ts">
// SettingsPage — extension-локальные настройки + RAWG API ключ
// (хранится через `arrancador.config.set_rawg_key` в backend config).

import { computed, onMounted, ref, watch } from "vue";
import { Eye, EyeOff, ExternalLink } from "lucide-vue-next";

import { SettingsRow, Toggle } from "@kepler/visuals";

import { arrancadorApi } from "../lib/arrancadorApi";

const STORAGE_KEY = "arrancador-extension-settings-v1";

interface ExtensionSettings {
  showLegacyHints: boolean;
  defaultSection: "library" | "scan" | "stats";
}

const defaults: ExtensionSettings = {
  showLegacyHints: true,
  defaultSection: "library",
};

const settings = ref<ExtensionSettings>({ ...defaults });
const savedAt = ref<number | null>(null);

function loadFromStorage(): ExtensionSettings {
  if (typeof window === "undefined") return { ...defaults };
  try {
    const raw = window.localStorage.getItem(STORAGE_KEY);
    if (!raw) return { ...defaults };
    const parsed = JSON.parse(raw) as Partial<ExtensionSettings>;
    return {
      showLegacyHints:
        typeof parsed.showLegacyHints === "boolean"
          ? parsed.showLegacyHints
          : defaults.showLegacyHints,
      defaultSection:
        parsed.defaultSection === "library" ||
        parsed.defaultSection === "scan" ||
        parsed.defaultSection === "stats"
          ? parsed.defaultSection
          : defaults.defaultSection,
    };
  } catch {
    return { ...defaults };
  }
}

function persist(next: ExtensionSettings) {
  if (typeof window === "undefined") return;
  try {
    window.localStorage.setItem(STORAGE_KEY, JSON.stringify(next));
    savedAt.value = Date.now();
  } catch {
    // ignore quota
  }
}

onMounted(() => {
  settings.value = loadFromStorage();
});

watch(
  settings,
  (next) => {
    persist(next);
  },
  { deep: true },
);

function resetDefaults() {
  settings.value = { ...defaults };
}

const savedLabel = computed(() => {
  if (!savedAt.value) return null;
  return "Сохранено";
});

// ---- RAWG API key ----

const rawgKey = ref("");
const rawgKeyLoaded = ref(false);
const rawgKeyVisible = ref(false);
const rawgKeySaving = ref(false);
const rawgKeyError = ref<string | null>(null);
const rawgKeyStatus = ref<string | null>(null);

const hasRawgKey = computed(
  () => rawgKeyLoaded.value && rawgKey.value.trim().length > 0,
);

async function loadRawgKey() {
  const api = arrancadorApi();
  if (!api) {
    rawgKeyError.value = "Bridge недоступен";
    rawgKeyLoaded.value = true;
    return;
  }
  try {
    const res = await api.config.getRawgKey();
    rawgKey.value = res.key ?? "";
  } catch (cause) {
    rawgKeyError.value =
      cause instanceof Error ? cause.message : "Не удалось загрузить ключ";
  } finally {
    rawgKeyLoaded.value = true;
  }
}

onMounted(loadRawgKey);

async function saveRawgKey() {
  const api = arrancadorApi();
  if (!api) return;
  rawgKeySaving.value = true;
  rawgKeyError.value = null;
  rawgKeyStatus.value = null;
  try {
    const res = await api.config.setRawgKey(rawgKey.value.trim());
    if (res.ok) {
      rawgKeyStatus.value = "Ключ сохранён";
      setTimeout(() => (rawgKeyStatus.value = null), 2500);
    } else {
      rawgKeyError.value = res.error ?? "Не удалось сохранить ключ";
    }
  } catch (cause) {
    rawgKeyError.value =
      cause instanceof Error ? cause.message : "Не удалось сохранить ключ";
  } finally {
    rawgKeySaving.value = false;
  }
}

function toggleRawgKeyVisibility() {
  rawgKeyVisible.value = !rawgKeyVisible.value;
}

function openRawgApiDocs() {
  // External URL → window.open позволит Electron открыть в системном браузере.
  window.open("https://rawg.io/apidocs", "_blank", "noopener,noreferrer");
}
</script>

<template>
  <section class="arrancador-page">
    <h1 class="arrancador-page__title">Настройки</h1>

    <div class="arrancador-settings">
      <SettingsRow
        title="RAWG API ключ"
        description="Используется для поиска metadata игр в каталоге. Получить бесплатно на rawg.io/apidocs."
      >
        <template #control>
          <div class="arrancador-rawg-key">
            <input
              v-model="rawgKey"
              :type="rawgKeyVisible ? 'text' : 'password'"
              placeholder="вставьте ключ…"
              class="arrancador-rawg-key__input"
              @blur="saveRawgKey"
            />
            <button
              type="button"
              class="arrancador-rawg-key__icon-btn"
              :title="rawgKeyVisible ? 'Скрыть' : 'Показать'"
              @click="toggleRawgKeyVisibility"
            >
              <EyeOff v-if="rawgKeyVisible" :size="14" />
              <Eye v-else :size="14" />
            </button>
            <button
              type="button"
              class="arrancador-rawg-key__icon-btn"
              title="Открыть rawg.io/apidocs"
              @click="openRawgApiDocs"
            >
              <ExternalLink :size="14" />
            </button>
          </div>
        </template>
      </SettingsRow>

      <div class="arrancador-rawg-key-status">
        <span v-if="!rawgKeyLoaded">Загрузка…</span>
        <span
          v-else-if="rawgKeyError"
          class="arrancador-rawg-key-status--error"
        >
          {{ rawgKeyError }}
        </span>
        <span v-else-if="rawgKeyStatus">{{ rawgKeyStatus }}</span>
        <span v-else-if="hasRawgKey">Ключ задан.</span>
        <span v-else>Ключ не задан.</span>
        <button
          v-if="rawgKeyLoaded && !rawgKeySaving"
          type="button"
          class="arrancador-rawg-key-status__save"
          :disabled="rawgKeySaving"
          @click="saveRawgKey"
        >
          Сохранить
        </button>
      </div>

      <SettingsRow
        title="Показывать подсказки"
        description="Информационные баннеры на страницах extension'а."
      >
        <template #control>
          <Toggle
            v-model="settings.showLegacyHints"
            aria-label="Показывать подсказки"
          />
        </template>
      </SettingsRow>

      <SettingsRow
        title="Стартовый раздел"
        description="Какой раздел открывается первым при запуске extension'а."
      >
        <template #control>
          <select
            v-model="settings.defaultSection"
            class="arrancador-settings__select"
          >
            <option value="library">Библиотека</option>
            <option value="scan">Сканер</option>
            <option value="stats">Статистика</option>
          </select>
        </template>
      </SettingsRow>

      <div class="arrancador-settings__actions">
        <button
          type="button"
          class="arrancador-settings__btn"
          @click="resetDefaults"
        >
          Сбросить к умолчаниям
        </button>
        <span v-if="savedLabel" class="arrancador-settings__saved">
          {{ savedLabel }}
        </span>
      </div>
    </div>
  </section>
</template>

<style scoped>
.arrancador-rawg-key {
  display: inline-flex;
  align-items: center;
  gap: 4px;
}

.arrancador-rawg-key__input {
  width: 240px;
  height: 30px;
  padding: 0 10px;
  border: 1px solid var(--border);
  border-radius: var(--radius-input);
  background: var(--background);
  color: var(--foreground);
  outline: none;
}

.arrancador-rawg-key__input:focus {
  border-color: var(--accent);
}

.arrancador-rawg-key__icon-btn {
  display: inline-flex;
  align-items: center;
  justify-content: center;
  width: 28px;
  height: 28px;
  border-radius: var(--radius-input);
  color: var(--muted-foreground);
  background: transparent;
  transition: background 120ms var(--easing-standard);
}

.arrancador-rawg-key__icon-btn:hover {
  background: var(--muted);
  color: var(--foreground);
}

.arrancador-rawg-key-status {
  display: flex;
  align-items: center;
  gap: 12px;
  margin: 4px 0 8px;
  font-size: 12px;
  color: var(--muted-foreground);
}

.arrancador-rawg-key-status--error {
  color: var(--destructive-foreground, var(--destructive));
}

.arrancador-rawg-key-status__save {
  height: 24px;
  padding: 0 10px;
  border: 1px solid var(--border);
  border-radius: var(--radius-input);
  background: var(--card);
  color: var(--foreground);
  font-size: 11px;
}

.arrancador-rawg-key-status__save:hover {
  background: var(--muted);
}
</style>
