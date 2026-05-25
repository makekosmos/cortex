<script setup lang="ts">
// FocusTab — блок-листы доменов + системный демон + inline editor.
// State и handlers из composable `useFocusTab`. Owner-у достаточно передать
// `intro` для верхнего hero-блока.

import { onBeforeUnmount, onMounted, type Component } from "vue";
import { BlocklistCard, SettingsAdvancedIntro } from "@kosmos/visuals";
import { ICON_CHOICES, useFocusTab } from "../composables/useFocusTab";

interface IntroDescriptor {
  icon: Component;
  label: string;
  description?: string;
  introImage?: string;
  iconGradient?: { from?: string; to?: string };
}

defineProps<{ intro: IntroDescriptor | null }>();

const {
  focusBlocklists,
  focusLoading,
  focusError,
  focusBackendMissing,
  focusActive,
  focusActiveBlocklist,
  focusBusy,
  focusServiceStatus,
  focusServiceBusy,
  focusServiceError,
  focusEditing,
  focusEditingId,
  focusDraftName,
  focusDraftDomains,
  focusDraftIcon,
  focusDraftError,
  focusDragOver,
  focusDraftParsed,
  mentionOpen,
  mentionCandidates,
  mentionTextareaRef,
  mentionHighlight,
  loadBlocklists,
  loadActiveState,
  refreshFocusServiceStatus,
  installFocusService,
  uninstallFocusService,
  openCreateBlocklist,
  openEditBlocklist,
  cancelCreateBlocklist,
  onCreateBlocklist,
  onDomainsInput,
  insertMention,
  onMentionKey,
  onDeleteBlocklist,
  onDeactivate,
  onActivate,
  onDraftDragOver,
  onDraftDragLeave,
  onDraftDrop,
} = useFocusTab();

let unsubscribeFocusServiceStatus: (() => void) | null = null;

onMounted(() => {
  focusBackendMissing.value = false;
  void loadBlocklists();
  void loadActiveState();
  void refreshFocusServiceStatus();
  unsubscribeFocusServiceStatus = window.kepler.focusService.onStatusChanged(() => {
    void refreshFocusServiceStatus();
  });
});

onBeforeUnmount(() => {
  unsubscribeFocusServiceStatus?.();
  unsubscribeFocusServiceStatus = null;
});

// `mentionTextareaRef` назначается через `ref="mentionTextareaRef"` в template
// — Vue компилятор связывает binding, но TS не видит usage без явной экспозиции.
defineExpose({ mentionTextareaRef });
</script>

<template>
  <section class="advanced-page kosmos-scroll">
    <SettingsAdvancedIntro
      v-if="intro"
      :icon="intro.icon"
      :title="intro.label"
      :description="intro.description"
      :image-src="intro.introImage"
      :icon-from="intro.iconGradient?.from"
      :icon-to="intro.iconGradient?.to"
    />

    <div class="advanced-page__body">
      <div v-if="focusBackendMissing" class="error-banner">
        Backend ещё не поддерживает focus.*. Обнови Kepler.
      </div>
      <div v-if="focusError" class="error-banner">{{ focusError }}</div>

      <div class="rows kosmos-scroll">
        <!-- Демон фокус-режима — устранит UAC при каждом включении блокировки -->
        <div class="row focus-service-row">
          <div class="row-label">
            <div class="label">Системный демон</div>
            <div
              v-if="focusServiceStatus.installed && focusServiceStatus.running"
              class="hint focus-service-hint-ok"
            >
              Установлен и работает — блокировка включается без запроса прав администратора.
            </div>
            <div v-else-if="focusServiceStatus.installed" class="hint">
              Установлен, но не запущен. Перезапусти Windows или нажми «Переустановить».
            </div>
            <div v-else class="hint">
              Без демона Windows запрашивает права администратора при каждом включении блокировки.
              Установи один раз — и все последующие активации будут без UAC.
            </div>
            <div v-if="focusServiceError" class="error">{{ focusServiceError }}</div>
          </div>
          <div class="row-actions">
            <button
              v-if="!focusServiceStatus.installed"
              type="button"
              class="btn primary"
              :disabled="focusServiceBusy === 'install'"
              @click="installFocusService"
            >
              {{ focusServiceBusy === "install" ? "Установка…" : "Установить" }}
            </button>
            <template v-else>
              <button
                type="button"
                class="btn ghost"
                :disabled="focusServiceBusy === 'install'"
                title="Переустановить если служба перестала работать"
                @click="installFocusService"
              >
                {{ focusServiceBusy === "install" ? "Установка…" : "Переустановить" }}
              </button>
              <button
                type="button"
                class="btn ghost danger"
                :disabled="focusServiceBusy === 'uninstall'"
                @click="uninstallFocusService"
              >
                {{ focusServiceBusy === "uninstall" ? "Удаление…" : "Удалить" }}
              </button>
            </template>
          </div>
        </div>

        <!-- Активная блокировка -->
        <div class="row focus-active-row">
          <div class="row-label">
            <div class="label">Активная блокировка</div>
            <div v-if="focusActive.active && focusActiveBlocklist" class="hint">
              «{{ focusActiveBlocklist.name }}» —
              {{ focusActiveBlocklist.domains.length }} доменов
            </div>
            <div v-else-if="focusActive.active" class="hint">
              Включена (блок-лист id: {{ focusActive.blocklist_id }})
            </div>
            <div v-else class="hint">Сейчас блокировка не активна</div>
          </div>
          <div class="row-actions">
            <button
              type="button"
              class="btn ghost danger"
              :disabled="!focusActive.active || focusBusy === '__deactivate__'"
              @click="onDeactivate"
            >
              {{ focusBusy === "__deactivate__" ? "Отключение…" : "Отключить" }}
            </button>
          </div>
        </div>

        <!-- Список блок-листов как grid карточек -->
        <div class="ext-section-title">Блок-листы</div>

        <div v-if="focusLoading" class="empty">Загрузка…</div>

        <div v-else class="focus-grid">
          <BlocklistCard
            v-for="bl in focusBlocklists"
            :key="bl.id"
            :name="bl.name"
            :domains="bl.domains"
            :icon="bl.icon ?? ''"
            :preset="bl.preset ?? false"
            :active="focusActive.active && focusActive.blocklist_id === bl.id"
            :count="bl.domains.length"
            @click="openEditBlocklist(bl)"
            @delete="onDeleteBlocklist(bl.id)"
          />
          <button
            type="button"
            class="add-card"
            :disabled="focusEditing || focusBackendMissing"
            @click="openCreateBlocklist"
          >
            + Создать блок-лист
          </button>
        </div>

        <!-- Edit modal/inline form -->
        <div v-if="focusEditing" class="focus-create-form">
          <div class="focus-create-row">
            <label class="label" for="focus-blocklist-name">Название</label>
            <input
              id="focus-blocklist-name"
              v-model="focusDraftName"
              type="text"
              class="focus-input"
              placeholder="Например: Соцсети"
              autocomplete="off"
            />
          </div>

          <div class="focus-create-row">
            <label class="label">Иконка</label>
            <div class="icon-picker">
              <button
                v-for="ic in ICON_CHOICES"
                :key="ic"
                type="button"
                class="icon-choice"
                :class="{ selected: focusDraftIcon === ic }"
                @click="focusDraftIcon = focusDraftIcon === ic ? '' : ic"
              >
                {{ ic }}
              </button>
            </div>
          </div>

          <div class="focus-create-row mention-host">
            <label class="label" for="focus-blocklist-domains">
              Домены (по одному на строку, # — комментарии, @ — ссылка на другой блок-лист)
            </label>
            <textarea
              id="focus-blocklist-domains"
              ref="mentionTextareaRef"
              v-model="focusDraftDomains"
              class="focus-textarea"
              :class="{ 'drag-over': focusDragOver }"
              spellcheck="false"
              rows="8"
              placeholder="# перетащи .txt сюда или вставь список&#10;twitter.com&#10;@preset:distractions"
              @input="onDomainsInput"
              @keydown="onMentionKey"
              @dragover="onDraftDragOver"
              @dragleave="onDraftDragLeave"
              @drop="onDraftDrop"
            />
            <div v-if="mentionOpen && mentionCandidates.length > 0" class="mention-dropdown">
              <button
                v-for="(bl, i) in mentionCandidates"
                :key="bl.id"
                type="button"
                class="mention-item"
                :class="{ active: i === mentionHighlight }"
                @mousedown.prevent="insertMention(bl)"
              >
                <span class="mention-icon">{{ bl.icon || "📁" }}</span>
                <span class="mention-name">{{ bl.name }}</span>
                <code class="mention-id">@{{ bl.id }}</code>
              </button>
            </div>
            <div class="hint focus-parse-stats">
              Записей: {{ focusDraftParsed.domains.length }}
              <span v-if="focusDraftParsed.kind === 'raw'" class="ext-author">
                · содержит ссылки
              </span>
              <span v-if="focusDraftParsed.invalid.length > 0" class="focus-invalid-count">
                · невалидных: {{ focusDraftParsed.invalid.length }}
              </span>
            </div>
          </div>
          <div v-if="focusDraftError" class="error">{{ focusDraftError }}</div>
          <div class="focus-create-actions">
            <button
              type="button"
              class="btn ghost"
              :disabled="focusBusy === '__create__'"
              @click="cancelCreateBlocklist"
            >
              Отмена
            </button>
            <button
              v-if="focusEditingId && !focusActive.active"
              type="button"
              class="btn"
              :disabled="focusBusy === '__create__' || focusBackendMissing"
              @click="onActivate(focusEditingId)"
            >
              Включить
            </button>
            <button
              type="button"
              class="btn"
              :disabled="focusBusy === '__create__' || focusBackendMissing"
              @click="onCreateBlocklist"
            >
              {{ focusBusy === "__create__" ? "Сохранение…" : "Сохранить" }}
            </button>
          </div>
        </div>
      </div>
    </div>
  </section>
</template>

<style scoped>
/* Tab-specific Focus CSS (мигрировано из родительского scoped-style). */
.focus-active-row,
.focus-service-row {
  background: var(--settings-list-background);
}

.focus-service-hint-ok {
  color: color-mix(in srgb, #4ade80 65%, var(--foreground) 35%);
}

.focus-item {
  align-items: flex-start;
}

.focus-domains-preview {
  font-family: var(--font-mono, ui-monospace, monospace);
  font-size: 11px;
  color: color-mix(in srgb, var(--foreground) 55%, transparent);
  word-break: break-all;
}

.focus-create-form {
  display: flex;
  flex-direction: column;
  gap: 10px;
  padding: 14px;
  margin-top: 6px;
  border-radius: 8px;
  background: color-mix(in srgb, var(--foreground) 5%, transparent);
  border: 1px solid color-mix(in srgb, var(--foreground) 10%, transparent);
}

.focus-create-row {
  display: flex;
  flex-direction: column;
  gap: 6px;
}

.focus-input {
  font: inherit;
  font-size: 12px;
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

.focus-textarea {
  font-family: var(--font-mono, ui-monospace, monospace);
  font-size: 12px;
  padding: 8px 10px;
  border-radius: 6px;
  border: 1px solid color-mix(in srgb, var(--foreground) 14%, transparent);
  background: color-mix(in srgb, oklch(0.04 0 0) 60%, transparent);
  color: var(--foreground);
  outline: none;
  resize: vertical;
  min-height: 120px;
  line-height: 1.5;
}

.focus-textarea:focus {
  border-color: color-mix(in srgb, var(--accent, oklch(0.7 0.18 250)) 60%, transparent);
}

.focus-textarea.drag-over {
  border-color: var(--accent, oklch(0.7 0.18 250));
  background: color-mix(in srgb, var(--accent, oklch(0.7 0.18 250)) 12%, transparent);
}

.focus-parse-stats {
  margin-top: 2px;
}

.focus-invalid-count {
  color: oklch(0.65 0.22 25);
}

.focus-create-actions {
  display: flex;
  justify-content: flex-end;
  gap: 8px;
}

.focus-grid {
  display: grid;
  grid-template-columns: repeat(auto-fill, minmax(220px, 1fr));
  gap: 12px;
  padding: 12px 4px;
}

.add-card {
  min-height: 140px;
  border: 2px dashed color-mix(in srgb, var(--foreground) 16%, transparent);
  border-radius: 12px;
  background: transparent;
  color: color-mix(in srgb, var(--foreground) 55%, transparent);
  cursor: pointer;
  transition:
    border-color 120ms ease,
    color 120ms ease;
  font: inherit;
  font-size: 13px;
  font-weight: 500;
}

.add-card:hover:not(:disabled) {
  border-color: var(--accent, oklch(0.7 0.18 250));
  color: var(--foreground);
}

.add-card:disabled {
  opacity: 0.5;
  cursor: not-allowed;
}

.icon-picker {
  display: grid;
  grid-template-columns: repeat(10, 1fr);
  gap: 4px;
  max-width: 360px;
}

.icon-choice {
  font: inherit;
  font-size: 16px;
  width: 28px;
  height: 28px;
  display: flex;
  align-items: center;
  justify-content: center;
  border: 1px solid transparent;
  border-radius: 6px;
  background: color-mix(in srgb, var(--foreground) 4%, transparent);
  cursor: pointer;
}

.icon-choice:hover {
  background: color-mix(in srgb, var(--foreground) 10%, transparent);
}

.icon-choice.selected {
  border-color: var(--accent, oklch(0.7 0.18 250));
  background: color-mix(in srgb, var(--accent, oklch(0.7 0.18 250)) 18%, transparent);
}

.mention-host {
  position: relative;
}

.mention-dropdown {
  position: absolute;
  left: 0;
  right: 0;
  bottom: 32px;
  z-index: 20;
  background: color-mix(in srgb, oklch(0.04 0 0) 96%, transparent);
  border: 1px solid color-mix(in srgb, var(--foreground) 14%, transparent);
  border-radius: 8px;
  box-shadow: 0 8px 24px rgba(0, 0, 0, 0.4);
  padding: 4px;
  display: flex;
  flex-direction: column;
  gap: 2px;
  max-height: 220px;
  overflow-y: auto;
}

.mention-item {
  display: flex;
  align-items: center;
  gap: 8px;
  padding: 6px 8px;
  border: none;
  background: transparent;
  color: var(--foreground);
  border-radius: 6px;
  cursor: pointer;
  font: inherit;
  font-size: 12px;
  text-align: left;
}

.mention-item:hover,
.mention-item.active {
  background: color-mix(in srgb, var(--foreground) 10%, transparent);
}

.mention-icon {
  font-size: 14px;
}

.mention-name {
  flex: 1;
  min-width: 0;
  white-space: nowrap;
  overflow: hidden;
  text-overflow: ellipsis;
}

.mention-id {
  font-size: 10px;
  font-family: var(--font-mono, ui-monospace, monospace);
  color: color-mix(in srgb, var(--foreground) 50%, transparent);
}
</style>
