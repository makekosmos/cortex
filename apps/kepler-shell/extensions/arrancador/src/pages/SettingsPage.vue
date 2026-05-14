<script setup lang="ts">
// SettingsPage — минимальные настройки extension'а.
//
// Адаптация vs `apps/arrancador/src-vue/pages/SettingsPage.vue`:
//   - убраны секции, требующие native API: AutoStart, ARK connection panel
//     (open database / directory), Backup directory chooser, Compression,
//     SQOBA manifest, RAWG API key, theme switcher (theme — global shell-level).
//   - оставлены только локальные user-preference переключатели, которые
//     сохраняются в `localStorage`. Native-зависимые секции — Phase 5+.

import { computed, onMounted, ref, watch } from "vue";

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
    // Ignore quota failures.
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
</script>

<template>
  <section class="arrancador-page">
    <h1 class="arrancador-page__title">Настройки</h1>

    <p class="arrancador-page__hint">
      Полный набор настроек (резервные копии, RAWG-ключ, авто-старт)
      доступен только в legacy Arrancador.exe. Здесь — только локальные
      настройки extension'а в localStorage.
    </p>

    <div class="arrancador-settings">
      <label class="arrancador-settings__row">
        <input
          v-model="settings.showLegacyHints"
          type="checkbox"
          class="arrancador-settings__checkbox"
        />
        <span class="arrancador-settings__label">
          Показывать подсказки про legacy Arrancador.exe
        </span>
      </label>

      <div class="arrancador-settings__row">
        <span class="arrancador-settings__label">Стартовый раздел</span>
        <select
          v-model="settings.defaultSection"
          class="arrancador-settings__select"
        >
          <option value="library">Библиотека</option>
          <option value="scan">Сканер</option>
          <option value="stats">Статистика</option>
        </select>
      </div>

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
