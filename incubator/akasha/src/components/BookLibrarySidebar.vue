<script setup lang="ts">
import { Plus } from "@lucide/vue";
import { PhBookOpen, PhBooks, PhCheckCircle, PhHouse } from "@phosphor-icons/vue";
import { SettingsSearchInput, SettingsSidebar, SettingsSidebarButton } from "@kosmos/visuals";
import type { Component } from "vue";
import type { LibrarySection } from "../lib/libraryView";
import type { BookRecord } from "../lib/bookStorage";

const props = defineProps<{
  activeSection: LibrarySection;
  books: BookRecord[];
  loading: boolean;
  searchQuery: string;
}>();

const emit = defineEmits<{
  importRequest: [];
  "update:activeSection": [section: LibrarySection];
  "update:searchQuery": [query: string];
}>();

const sidebarSections: Array<{
  id: LibrarySection;
  label: string;
  icon: Component;
}> = [
  { id: "home", label: "Главная", icon: PhHouse },
  { id: "all", label: "Все книги", icon: PhBooks },
  { id: "reading", label: "Читаю", icon: PhBookOpen },
  { id: "finished", label: "Завершённые", icon: PhCheckCircle },
];

function onSearchChange(value: string) {
  emit("update:searchQuery", value);
}
</script>

<template>
  <SettingsSidebar>
    <div class="book-library-sidebar">
      <div class="book-library-sidebar__search">
        <SettingsSearchInput
          :model-value="searchQuery"
          placeholder="Поиск"
          @update:model-value="onSearchChange"
        />
      </div>

      <nav class="book-library-sidebar__nav kosmos-scroll" aria-label="Разделы библиотеки">
        <SettingsSidebarButton
          v-for="section in sidebarSections"
          :key="section.id"
          :active="activeSection === section.id"
          :icon="section.icon"
          :label="section.label"
          type="button"
          @click="emit('update:activeSection', section.id)"
        />
      </nav>

      <button
        class="book-library-sidebar__add"
        type="button"
        :disabled="loading"
        @click="emit('importRequest')"
      >
        <Plus :size="17" :stroke-width="2" aria-hidden="true" />
        <span>{{ loading ? "Добавляю" : "Добавить EPUB" }}</span>
      </button>
    </div>
  </SettingsSidebar>
</template>

<style scoped>
.book-library-sidebar {
  display: flex;
  min-height: 0;
  flex: 1;
  flex-direction: column;
  gap: 18px;
  padding: 14px 8px 12px;
}

.book-library-sidebar__search {
  padding: 0 4px;
}

.book-library-sidebar__nav {
  display: flex;
  min-height: 0;
  flex-direction: column;
  gap: 3px;
  overflow-x: hidden;
  overflow-y: auto;
  padding: 0 4px 8px;
}

.book-library-sidebar__add {
  display: flex;
  align-items: center;
  width: 100%;
  border: 0;
  cursor: pointer;
  font-family: var(--font-sans);
  -webkit-app-region: no-drag;
}

.book-library-sidebar__add {
  justify-content: center;
  gap: 8px;
  height: 44px;
  margin-top: auto;
  border-radius: 8px;
  background: var(--primary);
  color: var(--primary-foreground);
  font-size: 13px;
  font-weight: 700;
}

.book-library-sidebar__add:hover:not(:disabled) {
  background: color-mix(in srgb, var(--primary) 88%, var(--foreground));
}

.book-library-sidebar__add:disabled {
  cursor: wait;
  opacity: 0.65;
}
</style>
