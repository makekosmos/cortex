<script setup lang="ts">
// FocusTab — orchestrator: provides FocusTabKey (singleton state +
// handlers), рендерит service-row + active-row + grid карточек + inline
// editor (FocusBlocklistEditor через inject).

import { onBeforeUnmount, onMounted, provide } from "vue";
import { BlocklistCard } from "@kosmos/visuals";
import AdvancedPageLayout, {
  type IntroDescriptor,
} from "../components/AdvancedPageLayout.vue";
import LegacyRow from "../components/LegacyRow.vue";
import { FocusTabKey, useFocusTab } from "../composables/useFocusTab";
import FocusBlocklistEditor from "./FocusBlocklistEditor.vue";

defineProps<{ intro: IntroDescriptor | null }>();

const ctx = useFocusTab();
provide(FocusTabKey, ctx);

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
  loadBlocklists,
  loadActiveState,
  refreshFocusServiceStatus,
  installFocusService,
  uninstallFocusService,
  openCreateBlocklist,
  openEditBlocklist,
  onDeleteBlocklist,
  onDeactivate,
} = ctx;

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
</script>

<template>
  <AdvancedPageLayout :intro="intro">
    <div v-if="focusBackendMissing" class="error-banner">
        Backend ещё не поддерживает focus.*. Обнови Kepler.
      </div>
      <div v-if="focusError" class="error-banner">{{ focusError }}</div>

      <div class="rows kosmos-scroll">
        <!-- Демон фокус-режима — устранит UAC при каждом включении блокировки -->
        <LegacyRow class="focus-service-row" title="Системный демон" :error="focusServiceError">
          <template #hint>
            <span
              v-if="focusServiceStatus.installed && focusServiceStatus.running"
              class="focus-service-hint-ok"
            >
              Установлен и работает — блокировка включается без запроса прав администратора.
            </span>
            <template v-else-if="focusServiceStatus.installed">
              Установлен, но не запущен. Перезапусти Windows или нажми «Переустановить».
            </template>
            <template v-else>
              Без демона Windows запрашивает права администратора при каждом включении блокировки.
              Установи один раз — и все последующие активации будут без UAC.
            </template>
          </template>
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
        </LegacyRow>

        <!-- Активная блокировка -->
        <LegacyRow class="focus-active-row" title="Активная блокировка">
          <template #hint>
            <template v-if="focusActive.active && focusActiveBlocklist">
              «{{ focusActiveBlocklist.name }}» — {{ focusActiveBlocklist.domains.length }} доменов
            </template>
            <template v-else-if="focusActive.active">
              Включена (блок-лист id: {{ focusActive.blocklist_id }})
            </template>
            <template v-else>Сейчас блокировка не активна</template>
          </template>
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
        </LegacyRow>

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
        <FocusBlocklistEditor v-if="focusEditing" />
    </div>
  </AdvancedPageLayout>
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

/* Blocklist-editor CSS переехал в FocusBlocklistEditor.vue. */

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

</style>
