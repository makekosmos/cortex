<script setup lang="ts">
import type { CommandSnapshotNode } from "../../shared/command-ipc";
import CommandActionPanel from "./CommandActionPanel.vue";
import CommandDetailView from "./CommandDetailView.vue";
import { useCommandListView } from "./useCommandListView";

const props = defineProps<{
  root: CommandSnapshotNode;
  sessionId: string;
}>();

const {
  query,
  dropdownValue,
  detail,
  actionStatus,
  dropdown,
  visibleSections,
  visibleItems,
  selectedItem,
  placeholder,
  emptyMessage,
  isLoading,
  activeActions,
  executeAction,
  executeDropdownChange,
  updateSearch,
  selectItem,
} = useCommandListView(props);
</script>

<template>
  <section class="command-list-view" aria-label="Список команд">
    <div class="command-list-view__main">
      <div class="command-list-view__search">
        <input
          :value="query"
          class="command-list-view__input"
          type="search"
          :placeholder="placeholder"
          aria-label="Поиск"
          @input="updateSearch"
        />
        <select
          v-if="dropdown"
          v-model="dropdownValue"
          class="command-list-view__dropdown"
          :aria-label="dropdown.placeholder ?? 'Фильтр'"
          @change="executeDropdownChange"
        >
          <template v-for="section in dropdown.sections" :key="section.id">
            <template v-if="section.title">
              <optgroup :label="section.title">
                <option
                  v-for="option in section.options"
                  :key="`${section.id}:${option.value}`"
                  :value="option.value"
                >
                  {{ option.title }}
                </option>
              </optgroup>
            </template>
            <template v-else>
              <option
                v-for="option in section.options"
                :key="`${section.id}:${option.value}`"
                :value="option.value"
              >
                {{ option.title }}
              </option>
            </template>
          </template>
        </select>
      </div>

      <div class="command-list-view__items kosmos-scroll">
        <p v-if="isLoading" class="command-list-view__loading">Загрузка...</p>
        <section
          v-for="section in visibleSections"
          :key="section.id"
          class="command-list-view__section"
        >
          <h2 v-if="section.title" class="command-list-view__section-title">{{ section.title }}</h2>
          <button
            v-for="item in section.items"
            :key="item.id"
            v-memo="[
              item.id,
              item.icon,
              item.title,
              item.subtitle,
              item.accessories,
              item.id === selectedItem?.id,
            ]"
            class="command-list-view__item"
            :class="{ 'command-list-view__item--selected': item.id === selectedItem?.id }"
            type="button"
            @click="selectItem(item.id)"
          >
            <img
              v-if="item.icon"
              class="command-list-view__icon"
              :src="item.icon"
              :alt="item.title"
            />
            <span class="command-list-view__item-text">
              <span class="command-list-view__title">{{ item.title }}</span>
              <span v-if="item.subtitle" class="command-list-view__subtitle">{{
                item.subtitle
              }}</span>
            </span>
            <span v-if="item.accessories.length > 0" class="command-list-view__accessories">
              <span
                v-for="accessory in item.accessories"
                :key="`${item.id}:${accessory}`"
                class="command-list-view__accessory"
              >
                {{ accessory }}
              </span>
            </span>
          </button>
        </section>

        <p v-if="visibleItems.length === 0 && !isLoading" class="command-list-view__empty">
          {{ emptyMessage }}
        </p>
      </div>

      <footer class="command-list-view__footer">
        <CommandActionPanel :panel="activeActions" @execute="executeAction" />
        <p v-if="actionStatus" class="command-list-view__status">{{ actionStatus }}</p>
      </footer>
    </div>

    <CommandDetailView :detail="detail" />
  </section>
</template>

<style scoped>
.command-list-view {
  display: flex;
  min-height: 0;
  min-width: 0;
  flex: 1;
  overflow: hidden;
}

.command-list-view__main {
  display: flex;
  min-width: 0;
  flex: 1;
  flex-direction: column;
}

.command-list-view__search {
  display: flex;
  flex-shrink: 0;
  gap: 8px;
  border-bottom: 1px solid var(--border);
  padding: 10px 12px;
}

.command-list-view__input {
  min-width: 0;
  flex: 1;
  height: 34px;
  border: 1px solid var(--input);
  border-radius: var(--radius-input);
  outline: none;
  background: color-mix(in srgb, var(--foreground) 5%, transparent);
  color: var(--foreground);
  padding: 0 12px;
  font-size: 13px;
}

.command-list-view__dropdown {
  width: min(210px, 36%);
  height: 34px;
  flex: 0 1 210px;
  border: 1px solid var(--input);
  border-radius: var(--radius-input);
  outline: none;
  background: color-mix(in srgb, var(--foreground) 5%, transparent);
  color: var(--foreground);
  padding: 0 10px;
  font-size: 13px;
}

.command-list-view__input:focus,
.command-list-view__dropdown:focus {
  border-color: var(--accent);
}

.command-list-view__items {
  min-height: 0;
  flex: 1;
  overflow: auto;
  padding: 8px;
}

.command-list-view__section {
  display: grid;
  gap: 4px;
}

.command-list-view__section + .command-list-view__section {
  margin-top: 8px;
}

.command-list-view__section-title {
  margin: 6px 10px 2px;
  color: var(--muted-foreground);
  font-size: 11px;
  font-weight: 700;
}

.command-list-view__item {
  display: flex;
  width: 100%;
  min-height: 42px;
  align-items: center;
  gap: 10px;
  border: 0;
  border-radius: var(--radius-input);
  background: transparent;
  color: var(--foreground);
  padding: 7px 10px;
  text-align: left;
}

.command-list-view__item--selected {
  background: color-mix(in srgb, var(--accent) 22%, transparent);
}

.command-list-view__item-text {
  display: grid;
  min-width: 0;
  flex: 1;
  gap: 2px;
}

.command-list-view__icon {
  width: 24px;
  height: 24px;
  flex: 0 0 24px;
  border-radius: var(--radius-input);
  object-fit: cover;
}

.command-list-view__accessories {
  display: flex;
  max-width: 42%;
  flex: 0 1 auto;
  flex-wrap: wrap;
  justify-content: flex-end;
  gap: 4px;
}

.command-list-view__accessory {
  min-width: 0;
  max-width: 160px;
  overflow: hidden;
  border: 1px solid var(--border);
  border-radius: var(--radius-input);
  color: var(--muted-foreground);
  padding: 3px 7px;
  text-overflow: ellipsis;
  white-space: nowrap;
  font-size: 11px;
}

.command-list-view__title,
.command-list-view__subtitle {
  overflow: hidden;
  text-overflow: ellipsis;
  white-space: nowrap;
}

.command-list-view__title {
  font-size: 13px;
  font-weight: 600;
}

.command-list-view__subtitle,
.command-list-view__empty,
.command-list-view__loading {
  color: var(--muted-foreground);
  font-size: 12px;
}

.command-list-view__empty,
.command-list-view__loading {
  margin: 18px 10px;
}

.command-list-view__footer {
  display: flex;
  min-height: 44px;
  align-items: center;
  gap: 10px;
  border-top: 1px solid var(--border);
  padding: 8px 10px;
}

.command-list-view__status {
  margin: 0;
  overflow: hidden;
  color: var(--muted-foreground);
  text-overflow: ellipsis;
  white-space: nowrap;
  font-size: 12px;
}
</style>
