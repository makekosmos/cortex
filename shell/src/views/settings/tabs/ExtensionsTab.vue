<script setup lang="ts">
// ExtensionsTab — marketplace catalog + installed extensions с
// install/update/revert/uninstall actions.

import { onMounted, type Component } from "vue";
import { SettingsAdvancedIntro } from "@kosmos/visuals";
import { useExtensionsTab } from "../composables/useExtensionsTab";

interface IntroDescriptor {
  icon: Component;
  label: string;
  description?: string;
  introImage?: string;
  iconGradient?: { from?: string; to?: string };
}

defineProps<{ intro: IntroDescriptor | null }>();

const {
  installed,
  extensionsError,
  busyExt,
  marketLoading,
  marketError,
  installingId,
  availableInCatalog,
  loadExtensions,
  loadCatalog,
  catalogById,
  hasUpdate,
  onUpdate,
  onInstallNew,
  onRevert,
  onUninstall,
} = useExtensionsTab();

onMounted(() => {
  void loadExtensions();
  void loadCatalog();
});
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
      <div v-if="marketError" class="error-banner">{{ marketError }}</div>
      <div v-if="extensionsError" class="error-banner">{{ extensionsError }}</div>

      <div class="ext-list kosmos-scroll">
        <!-- Доступные из marketplace, ещё не установленные -->
        <div v-if="availableInCatalog.length > 0" class="ext-section-title">
          Доступные расширения
        </div>
        <div v-for="c in availableInCatalog" :key="`catalog-${c.id}`" class="ext-item">
          <img
            v-if="c.iconUrl"
            class="ext-icon"
            :src="c.iconUrl"
            alt=""
            @error="(e) => ((e.target as HTMLImageElement).style.display = 'none')"
          />
          <div v-else class="ext-icon ext-icon-fallback">
            {{ c.name.slice(0, 1) }}
          </div>
          <div class="ext-info">
            <div class="ext-name">{{ c.name }}</div>
            <div class="ext-meta">
              <span class="ext-version">v{{ c.version }}</span>
              <span v-if="c.author" class="ext-author">· {{ c.author }}</span>
            </div>
            <div v-if="c.description" class="ext-description">
              {{ c.description }}
            </div>
          </div>
          <div class="ext-actions">
            <button
              type="button"
              class="btn"
              :disabled="installingId === c.id"
              @click="onInstallNew(c)"
            >
              <template v-if="installingId === c.id">Установка…</template>
              <template v-else>Установить</template>
            </button>
          </div>
        </div>

        <!-- Установленные -->
        <div
          v-if="installed.length > 0 && availableInCatalog.length > 0"
          class="ext-section-title"
        >
          Установленные
        </div>
        <div v-if="installed.length === 0 && availableInCatalog.length === 0" class="empty">
          <template v-if="marketLoading">Загрузка каталога…</template>
          <template v-else>Расширений нет. Каталог пуст или недоступен.</template>
        </div>

        <div v-for="ext in installed" :key="ext.id" class="ext-item">
          <img v-if="ext.iconDataUri" class="ext-icon" :src="ext.iconDataUri" alt="" />
          <div v-else class="ext-icon ext-icon-fallback">
            {{ ext.name.slice(0, 1) }}
          </div>
          <div class="ext-info">
            <div class="ext-name">{{ ext.name }}</div>
            <div class="ext-meta">
              <span class="ext-version">v{{ ext.version ?? "—" }}</span>
              <span v-if="ext.source === 'dev'" class="ext-dev-badge">dev</span>
              <span v-if="ext.author" class="ext-author">· {{ ext.author }}</span>
              <span v-if="hasUpdate(ext) && ext.source !== 'dev'" class="ext-author">
                · доступно v{{ catalogById(ext.id)?.version }}
              </span>
              <span v-if="ext.backupCount > 0" class="ext-backups">
                · backup'ов: {{ ext.backupCount }}
              </span>
            </div>
            <div v-if="ext.description" class="ext-description">
              {{ ext.description }}
            </div>
          </div>
          <div class="ext-actions">
            <!-- Dev-source extension'ы (из repo) НЕ имеют update/revert/uninstall —
                 source code управляется git'ом, не Kepler installer'ом. -->
            <span v-if="ext.source === 'dev'" class="ext-dev-hint">
              источник: репозиторий
            </span>
            <template v-else>
              <button
                v-if="hasUpdate(ext)"
                type="button"
                class="btn"
                :disabled="installingId === ext.id || busyExt === ext.id"
                @click="onUpdate(ext)"
              >
                <template v-if="installingId === ext.id">Обновление…</template>
                <template v-else>Обновить</template>
              </button>
              <button
                v-if="ext.backupCount > 0"
                type="button"
                class="btn ghost"
                :disabled="busyExt === ext.id || installingId === ext.id"
                @click="onRevert(ext.id)"
              >
                Откатить
              </button>
              <button
                type="button"
                class="btn ghost danger"
                :disabled="busyExt === ext.id || installingId === ext.id"
                @click="onUninstall(ext.id)"
              >
                Удалить
              </button>
            </template>
          </div>
        </div>
      </div>

      <div class="ext-footer">
        <button
          type="button"
          class="btn ghost"
          :disabled="marketLoading"
          @click="loadCatalog(true)"
        >
          {{ marketLoading ? "Проверка…" : "Проверить обновления" }}
        </button>
      </div>
    </div>
  </section>
</template>

<style scoped>
/* Tab-specific CSS: мигрировано из родительского scoped-style SettingsView.
   `.ext-section-title` / `.ext-section-header` — shared между Extensions /
   Focus / Export tabs, поэтому живут в settings-shared.css, не здесь. */

.ext-footer {
  display: flex;
  justify-content: flex-start;
  padding: 12px 16px;
  border-top: 1px solid color-mix(in srgb, var(--foreground) 8%, transparent);
}

.market-header {
  display: flex;
  justify-content: space-between;
  align-items: center;
  padding: 10px 14px 0;
}

.ext-list {
  flex: 1;
  overflow-y: auto;
  padding: 16px;
  display: flex;
  flex-direction: column;
  gap: 8px;
}

.ext-item {
  display: flex;
  align-items: center;
  gap: 12px;
  padding: 12px;
  border-radius: 8px;
  background: color-mix(in srgb, var(--foreground) 4%, transparent);
}

.ext-icon {
  width: 40px;
  height: 40px;
  border-radius: 8px;
  object-fit: cover;
  flex-shrink: 0;
}

.ext-icon-fallback {
  background: color-mix(in srgb, var(--foreground) 10%, transparent);
  display: flex;
  align-items: center;
  justify-content: center;
  font-size: 16px;
  font-weight: 600;
  color: color-mix(in srgb, var(--foreground) 60%, transparent);
}

.ext-info {
  flex: 1;
  min-width: 0;
}

.ext-name {
  font-size: 13px;
  font-weight: 500;
  color: var(--foreground);
}

.ext-meta {
  font-size: 11px;
  color: color-mix(in srgb, var(--foreground) 50%, transparent);
  margin-top: 1px;
  /* gap 4px между токенами (версия / dev badge / автор / backup count) —
     раньше токены липли друг к другу, выглядело как один слово `v0.1.6·Kazui`. */
  display: flex;
  flex-wrap: wrap;
  column-gap: 4px;
  align-items: baseline;
}

.ext-description {
  font-size: 11px;
  color: color-mix(in srgb, var(--foreground) 65%, transparent);
  margin-top: 3px;
}

.ext-actions {
  display: flex;
  gap: 6px;
  flex-shrink: 0;
}
</style>
