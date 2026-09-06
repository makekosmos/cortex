<script setup lang="ts">
import { onMounted } from "vue";
import AdvancedPageLayout, { type IntroDescriptor } from "../components/AdvancedPageLayout.vue";
import { useExtensionsTab } from "../composables/useExtensionsTab";
import ExtensionItem from "./ExtensionItem.vue";

defineProps<{ intro: IntroDescriptor | null }>();

const {
  installed,
  extensionsError,
  extensionsLoading,
  busyExt,
  availableInCatalog,
  loadCatalog,
  catalogById,
  hasUpdate,
  install,
  onToggle,
} = useExtensionsTab();

onMounted(() => void loadCatalog());
</script>

<template>
  <AdvancedPageLayout :intro="intro">
    <div v-if="extensionsError" class="error-banner" role="alert">{{ extensionsError }}</div>

    <div class="ext-list kosmos-scroll">
      <div v-if="availableInCatalog.length" class="ext-section-title">Доступные приложения</div>
      <ExtensionItem
        v-for="item in availableInCatalog"
        :key="`catalog-${item.id}`"
        :name="item.name"
        :version="`v${item.version}`"
        :author="item.publisher"
      >
        <template #actions>
          <button type="button" class="btn" :disabled="busyExt === item.id" @click="install(item)">
            {{ busyExt === item.id ? "Установка…" : "Установить" }}
          </button>
        </template>
      </ExtensionItem>

      <div v-if="installed.length" class="ext-section-title">Установленные приложения</div>
      <div v-if="!installed.length && !availableInCatalog.length" class="empty">
        {{ extensionsLoading ? "Загрузка приложений…" : "Приложения пока не найдены." }}
      </div>
      <ExtensionItem
        v-for="item in installed"
        :key="item.id"
        :name="item.name"
        :version="`v${item.version}`"
        :author="item.publisher"
      >
        <template #meta>
          <span v-if="hasUpdate(item)" class="ext-author"
            >· доступно v{{ item.update_version }}</span
          >
          <span v-else-if="item.enabled === false" class="ext-author">· выключено</span>
        </template>
        <template #actions>
          <button
            v-if="hasUpdate(item)"
            type="button"
            class="btn"
            :disabled="busyExt === item.id"
            @click="
              install({
                ...item,
                version: catalogById(item.id)?.version ?? item.update_version ?? item.version,
              })
            "
          >
            {{ busyExt === item.id ? "Обновление…" : "Обновить" }}
          </button>
          <button
            type="button"
            class="btn ghost"
            :disabled="busyExt === item.id"
            @click="onToggle(item)"
          >
            {{ item.enabled === false ? "Включить" : "Выключить" }}
          </button>
        </template>
      </ExtensionItem>
    </div>

    <div class="ext-footer">
      <button
        type="button"
        class="btn ghost"
        :disabled="extensionsLoading"
        @click="loadCatalog(true)"
      >
        {{ extensionsLoading ? "Проверка…" : "Проверить обновления" }}
      </button>
    </div>
  </AdvancedPageLayout>
</template>

<style scoped>
.ext-footer {
  display: flex;
  justify-content: flex-start;
  padding: 12px 16px;
  border-top: 1px solid color-mix(in srgb, var(--foreground) 8%, transparent);
}

.ext-list {
  flex: 1;
  overflow-y: auto;
  padding: 16px;
  display: flex;
  flex-direction: column;
  gap: 8px;
}
</style>
