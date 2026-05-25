<script setup lang="ts">
// FocusBlocklistEditor — inline-форма создания/редактирования блок-листа
// (название, иконка, домены/refs textarea с @-mention autocomplete +
// drag-drop .txt файлов). Состояние подключается через `inject(FocusTabKey)`
// — provider живёт в `FocusTab.vue`.

import { inject } from "vue";
import { FocusTabKey, ICON_CHOICES } from "../composables/useFocusTab";

const ctx = inject(FocusTabKey);
if (!ctx) throw new Error("FocusBlocklistEditor requires FocusTabKey provider");

const {
  focusActive,
  focusBackendMissing,
  focusBusy,
  focusEditingId,
  focusDraftName,
  focusDraftIcon,
  focusDraftDomains,
  focusDraftError,
  focusDragOver,
  focusDraftParsed,
  mentionOpen,
  mentionCandidates,
  mentionTextareaRef,
  mentionHighlight,
  cancelCreateBlocklist,
  onCreateBlocklist,
  onDomainsInput,
  insertMention,
  onMentionKey,
  onActivate,
  onDraftDragOver,
  onDraftDragLeave,
  onDraftDrop,
} = ctx;

defineExpose({ mentionTextareaRef });
</script>

<template>
  <div class="focus-create-form">
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
        <span v-if="focusDraftParsed.kind === 'raw'" class="ext-author">· содержит ссылки</span>
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
</template>

<style scoped>
/* CSS форм редактора блок-листа — мигрировано из родительского FocusTab. */
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
