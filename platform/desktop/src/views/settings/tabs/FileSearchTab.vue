<script setup lang="ts">
// FileSearchTab — папки индексации, ignore patterns, и фичи индексатора.
// Toast api приходит сверху (provideToastHost в SettingsView), потому что
// прогресс индексации показывается toast'ом, который живёт дольше mount'а.

import { onBeforeUnmount, onMounted } from "vue";
import { Folder, Plus } from "@lucide/vue";
import { useToast } from "@kosmos/visuals";
import AdvancedPageLayout, { type IntroDescriptor } from "../components/AdvancedPageLayout.vue";
import LegacyRow from "../components/LegacyRow.vue";
import LegacyToggle from "../components/LegacyToggle.vue";
import { useFileSearchTab } from "../composables/useFileSearchTab";

defineProps<{ intro: IntroDescriptor | null }>();

const toast = useToast();

const {
  fileSearchSettings,
  fileSearchBusy,
  fileSearchError,
  fileSearchNewIgnore,
  fileSearchRootWarnings,
  loadFileSearchState,
  clearFileSearchPoll,
  onToggleFileSearchNoise,
  onToggleFileSearchGitignore,
  onToggleFileSearchHidden,
  onToggleFileSearchNtfs,
  onAddFileSearchScope,
  onRemoveFileSearchScope,
  onAddFileSearchIgnore,
  onRemoveFileSearchIgnore,
  onRescanFileSearch,
  onClearFileSearchCache,
} = useFileSearchTab(toast);

onMounted(() => {
  void loadFileSearchState();
});

onBeforeUnmount(() => {
  clearFileSearchPoll();
});
</script>

<template>
  <AdvancedPageLayout :intro="intro" body-class="rows">
    <div v-if="fileSearchError" class="error-banner">{{ fileSearchError }}</div>
    <!-- Regression M8 (2026-05-24): show loading state instead of
           misleading "no folders chosen" while settings load. -->
    <div v-if="fileSearchSettings === null && !fileSearchError" class="hint">
      Загрузка настроек поиска…
    </div>
    <div v-if="fileSearchSettings" class="row file-search-row file-search-row--scopes">
      <div class="row-label file-search-wide">
        <div class="file-search-section-header">
          <div class="label">Папки поиска</div>
          <button
            type="button"
            class="file-search-add-scope"
            :disabled="!fileSearchSettings || fileSearchBusy"
            aria-label="Добавить папку поиска"
            title="Добавить папку поиска"
            @click="onAddFileSearchScope"
          >
            <Plus :size="17" :stroke-width="2" />
          </button>
        </div>
        <!-- Regression L5 (2026-05-24): semantic list + button
               aria-label includes the scope so screen readers don't
               read "Удалить, Удалить, Удалить..." for N chips. -->
        <ul class="file-search-list" role="list">
          <li v-for="root in fileSearchSettings?.roots ?? []" :key="root" class="file-search-chip">
            <span class="file-search-chip__icon" aria-hidden="true">
              <Folder :size="16" :stroke-width="1.75" />
            </span>
            <code>{{ root }}</code>
            <button
              type="button"
              class="btn ghost danger"
              :disabled="fileSearchBusy"
              :aria-label="`Удалить папку поиска ${root}`"
              @click="onRemoveFileSearchScope(root)"
            >
              Удалить
            </button>
          </li>
          <li v-if="(fileSearchSettings?.roots.length ?? 0) === 0" class="hint">
            Папки не выбраны — поиск файлов ничего не индексирует.
          </li>
        </ul>
        <div v-if="fileSearchRootWarnings.length > 0" class="file-search-root-warnings">
          <div class="file-search-root-warnings__title">Предупреждения по корням</div>
          <div class="file-search-root-warnings__list">
            <div
              v-for="warning in fileSearchRootWarnings"
              :key="warning.path"
              class="file-search-root-warnings__item"
              :class="{
                'file-search-root-warnings__item--danger': warning.risk_level === 'danger',
              }"
            >
              <code>{{ warning.path }}</code>
              <span>
                {{ warning.risk_reasons[0] || "Требуется подтверждение" }}
              </span>
            </div>
          </div>
        </div>
      </div>
    </div>

    <div v-if="fileSearchSettings" class="row file-search-row">
      <div class="row-label file-search-wide">
        <div class="label">Шаблоны исключений</div>
        <div class="hint">
          Паттерны применяются к имени и пути файла. Примеры:
          <code>*.tmp</code>, <code>*.log</code>, <code>**/Cache/**</code>.
        </div>
        <ul class="file-search-list" role="list">
          <li
            v-for="pattern in fileSearchSettings?.ignore_patterns ?? []"
            :key="pattern"
            class="file-search-chip"
          >
            <code>{{ pattern }}</code>
            <button
              type="button"
              class="btn ghost danger"
              :disabled="fileSearchBusy"
              :aria-label="`Удалить шаблон ${pattern}`"
              @click="onRemoveFileSearchIgnore(pattern)"
            >
              Удалить
            </button>
          </li>
          <li v-if="(fileSearchSettings?.ignore_patterns.length ?? 0) === 0" class="hint">
            Пользовательских шаблонов пока нет.
          </li>
        </ul>
        <div class="file-search-add">
          <input
            v-model="fileSearchNewIgnore"
            class="focus-input"
            type="text"
            placeholder="*.tmp"
            :disabled="fileSearchBusy"
            @keydown.enter.prevent="onAddFileSearchIgnore"
          />
          <button
            type="button"
            class="btn"
            :disabled="fileSearchBusy || fileSearchNewIgnore.trim().length === 0"
            @click="onAddFileSearchIgnore"
          >
            Добавить
          </button>
        </div>
      </div>
    </div>

    <LegacyRow v-if="fileSearchSettings" title="Исключать шумные папки из поиска файлов">
      <template #hint>
        Kepler индексирует файлы на локальных дисках и пропускает
        <code>node_modules</code>, <code>.git</code>, сборки и временные каталоги. Изменение сразу
        запускает переиндексацию.
      </template>
      <LegacyToggle
        :checked="fileSearchSettings?.exclude_noisy_folders ?? true"
        :disabled="!fileSearchSettings || fileSearchBusy"
        @change="onToggleFileSearchNoise"
      />
    </LegacyRow>

    <LegacyRow v-if="fileSearchSettings" title="Учитывать .gitignore">
      <template #hint>
        Включено по умолчанию: Kepler пропускает файлы, которые проект сам считает мусором.
      </template>
      <LegacyToggle
        :checked="fileSearchSettings?.respect_gitignore ?? true"
        :disabled="!fileSearchSettings || fileSearchBusy"
        @change="onToggleFileSearchGitignore"
      />
    </LegacyRow>

    <LegacyRow v-if="fileSearchSettings" title="Показывать скрытые файлы">
      <template #hint>
        По умолчанию выключено, чтобы не засорять результаты dotfiles и системными скрытыми файлами.
      </template>
      <LegacyToggle
        :checked="fileSearchSettings?.include_hidden ?? false"
        :disabled="!fileSearchSettings || fileSearchBusy"
        @change="onToggleFileSearchHidden"
      />
    </LegacyRow>

    <LegacyRow v-if="fileSearchSettings" title="Ускоренный NTFS-режим">
      <template #hint>
        Быстрый scan для корней дисков. Если сервис недоступен или включён
        <code>.gitignore</code>, поиск автоматически использует обычный scan.
      </template>
      <!-- Regression H10 (2026-05-24): surface actual NTFS status, not just toggle position. -->
      <template #extra>
        <div
          v-if="
            fileSearchSettings?.ntfs_accelerated &&
            fileSearchSettings?.ntfs_status &&
            fileSearchSettings.ntfs_status !== 'disabled' &&
            fileSearchSettings.ntfs_status !== 'unknown'
          "
          class="hint"
          :class="{
            'ntfs-status-active': fileSearchSettings.ntfs_status === 'active',
            'ntfs-status-fallback':
              fileSearchSettings.ntfs_status === 'fallback' ||
              fileSearchSettings.ntfs_status === 'unavailable',
          }"
        >
          Реальный режим:
          <strong v-if="fileSearchSettings.ntfs_status === 'active'"> NTFS работает </strong>
          <strong v-else-if="fileSearchSettings.ntfs_status === 'fallback'"> обычный scan </strong>
          <strong v-else-if="fileSearchSettings.ntfs_status === 'unavailable'">
            NTFS недоступен
          </strong>
        </div>
      </template>
      <LegacyToggle
        :checked="fileSearchSettings?.ntfs_accelerated ?? false"
        :disabled="!fileSearchSettings || fileSearchBusy"
        @change="onToggleFileSearchNtfs"
      />
    </LegacyRow>

    <LegacyRow
      v-if="fileSearchSettings"
      title="Переиндексация"
      hint="Очистка удаляет только кеш поиска."
    >
      <div class="row-actions">
        <button
          type="button"
          class="btn ghost danger"
          :disabled="!fileSearchSettings || fileSearchBusy || fileSearchSettings?.scan_in_progress"
          @click="onClearFileSearchCache"
        >
          Очистить индекс
        </button>
        <button
          type="button"
          class="btn"
          :disabled="
            !fileSearchSettings ||
            fileSearchBusy ||
            fileSearchSettings?.scan_in_progress ||
            fileSearchSettings?.enabled === false
          "
          @click="onRescanFileSearch"
        >
          {{
            fileSearchBusy || fileSearchSettings?.scan_in_progress ? "Идёт…" : "Переиндексировать"
          }}
        </button>
      </div>
    </LegacyRow>
  </AdvancedPageLayout>
</template>

<style scoped>
/* Tab-specific File Search CSS (мигрировано из родительского scoped-style). */
.file-search-row {
  align-items: flex-start;
}

.file-search-row--scopes {
  position: relative;
}

.file-search-wide {
  width: 100%;
}

.file-search-section-header {
  display: flex;
  align-items: center;
  justify-content: space-between;
  gap: 12px;
  min-height: 28px;
}

.file-search-add-scope {
  display: inline-flex;
  align-items: center;
  justify-content: center;
  width: 24px;
  height: 24px;
  border-radius: 8px;
  border: 0;
  background: transparent;
  color: color-mix(in srgb, var(--foreground) 74%, transparent);
  cursor: default;
  transition:
    background 120ms ease,
    color 120ms ease;
}

.file-search-add-scope:hover:not(:disabled) {
  background: color-mix(in srgb, var(--foreground) 8%, transparent);
  color: var(--foreground);
}

.file-search-add-scope:disabled {
  cursor: default;
  opacity: 0.45;
}

.file-search-list {
  display: flex;
  flex-direction: column;
  gap: 6px;
  margin-top: 8px;
  list-style: none;
  padding: 0;
}

.file-search-chip {
  display: flex;
  align-items: center;
  gap: 10px;
  width: 100%;
  min-width: 0;
  padding: 9px 12px;
  border-radius: 10px;
  background: color-mix(in srgb, var(--foreground) 5%, transparent);
  border: 1px solid color-mix(in srgb, var(--foreground) 7%, transparent);
  transition:
    background 120ms ease,
    border-color 120ms ease;
}

.file-search-chip:hover {
  background: color-mix(in srgb, var(--foreground) 8%, transparent);
  border-color: color-mix(in srgb, var(--foreground) 12%, transparent);
}

.file-search-chip__icon {
  flex: 0 0 auto;
  color: color-mix(in srgb, var(--foreground) 55%, transparent);
  display: flex;
  align-items: center;
}

.file-search-chip code {
  flex: 1 1 auto;
  min-width: 0;
  overflow: hidden;
  text-overflow: ellipsis;
  white-space: nowrap;
  color: color-mix(in srgb, var(--foreground) 85%, transparent);
  font-family: var(--font-mono, ui-monospace, monospace);
  font-size: 0.75rem;
  direction: ltr;
}

/* Regression 2026-05-24-evening: delete button hidden until hover/focus. */
.file-search-chip .btn.ghost.danger {
  opacity: 0;
  transition: opacity 120ms ease;
  flex: 0 0 auto;
}

.file-search-chip:hover .btn.ghost.danger,
.file-search-chip:focus-within .btn.ghost.danger {
  opacity: 1;
}

.file-search-add {
  display: flex;
  align-items: center;
  gap: 8px;
  margin-top: 8px;
}

.file-search-add .focus-input {
  min-width: 180px;
}

.file-search-root-warnings {
  margin-top: 10px;
  padding: 10px 12px;
  border-radius: 8px;
  border: 1px solid color-mix(in srgb, oklch(0.7 0.12 85) 35%, transparent);
  background: color-mix(in srgb, oklch(0.7 0.12 85) 12%, transparent);
}

.file-search-root-warnings__title {
  font-size: 0.6875rem;
  text-transform: uppercase;
  letter-spacing: 0.04em;
  color: color-mix(in srgb, var(--foreground) 60%, transparent);
  margin-bottom: 6px;
}

.file-search-root-warnings__list {
  display: flex;
  flex-direction: column;
  gap: 6px;
}

.file-search-root-warnings__item {
  display: flex;
  gap: 8px;
  align-items: flex-start;
  min-width: 0;
  font-size: 0.75rem;
  color: color-mix(in srgb, var(--foreground) 85%, transparent);
}

.file-search-root-warnings__item code {
  flex: 0 0 auto;
  max-width: 42%;
  overflow: hidden;
  text-overflow: ellipsis;
  white-space: nowrap;
  font-family: var(--font-mono, ui-monospace, monospace);
}

.file-search-root-warnings__item span {
  min-width: 0;
  flex: 1 1 auto;
}

.file-search-root-warnings__item--danger {
  color: color-mix(in srgb, #fda4af 55%, var(--foreground) 45%);
}

.focus-input {
  font: inherit;
  font-size: 0.75rem;
  padding: 6px 10px;
  border-radius: 6px;
  border: 1px solid color-mix(in srgb, var(--foreground) 14%, transparent);
  background: color-mix(in srgb, oklch(0.04 0 0) 60%, transparent);
  color: var(--foreground);
  outline: none;
}

.focus-input:focus {
  border-color: color-mix(in srgb, var(--accent, oklch(0.7 0.18 250)) 60%, transparent);
}

.ntfs-status-active strong {
  color: color-mix(in srgb, #4ade80 65%, var(--foreground) 35%);
}

.ntfs-status-fallback strong {
  color: oklch(0.7 0.15 80);
}
</style>
