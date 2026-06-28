<script setup lang="ts">
import { computed, shallowRef, watch } from "vue";
import type { CommandSnapshotNode } from "../../shared/command-ipc";
import CommandActionPanel from "./CommandActionPanel.vue";
import CommandDetailView from "./CommandDetailView.vue";
import {
  listEmptyActions,
  listDropdown,
  listEmptyMessage,
  listFiltering,
  listIsLoading,
  listItems,
  listPlaceholder,
  listSearchCallbackNode,
  listSearchText,
  listSelectedItemId,
  listSelectionCallbackNode,
  listSections,
  matchesItem,
} from "./model-list";

const props = defineProps<{
  root: CommandSnapshotNode;
  sessionId: string;
}>();

const query = shallowRef("");
const selectedId = shallowRef<string | null>(null);
const dropdownValue = shallowRef("");
const pushedDetail = shallowRef<CommandSnapshotNode | null>(null);
const actionStatus = shallowRef<string | null>(null);

const items = computed(() => listItems(props.root));
const sections = computed(() => listSections(props.root));
const dropdown = computed(() => listDropdown(props.root));
const filtering = computed(() => listFiltering(props.root));
const searchCallbackNode = computed(() => listSearchCallbackNode(props.root));
const selectionCallbackNode = computed(() => listSelectionCallbackNode(props.root));
const visibleSections = computed(() =>
  sections.value
    .map((section) => ({
      ...section,
      items: filtering.value
        ? section.items.filter((item) => matchesItem(item, query.value))
        : section.items,
    }))
    .filter((section) => section.items.length > 0),
);
const visibleItems = computed(() => visibleSections.value.flatMap((section) => section.items));
const selectedItem = computed(
  () =>
    visibleItems.value.find((item) => item.id === selectedId.value) ??
    visibleItems.value[0] ??
    null,
);
const detail = computed(() => pushedDetail.value ?? selectedItem.value?.detail ?? null);
const placeholder = computed(() => listPlaceholder(props.root));
const emptyMessage = computed(() => listEmptyMessage(props.root));
const emptyActions = computed(() => listEmptyActions(props.root));
const isLoading = computed(() => listIsLoading(props.root));
const activeActions = computed(
  () =>
    selectedItem.value?.actions ??
    (visibleItems.value.length === 0 && !isLoading.value ? emptyActions.value : null),
);

watch(
  () => listSearchText(props.root),
  (next) => {
    query.value = next;
  },
  { immediate: true },
);

watch(
  () => listSelectedItemId(props.root),
  (next) => {
    selectedId.value = next;
  },
  { immediate: true },
);

watch(
  dropdown,
  (next) => {
    if (!next) {
      dropdownValue.value = "";
      return;
    }
    const nextValue = next.defaultValue ?? next.options[0]?.value ?? "";
    if (!next.options.some((option) => option.value === dropdownValue.value)) {
      dropdownValue.value = nextValue;
    }
  },
  { immediate: true },
);

let selectionSynced = false;

watch(
  visibleItems,
  (next) => {
    if (!next.some((item) => item.id === selectedId.value)) {
      const notify = selectionSynced;
      selectionSynced = true;
      void selectItem(next[0]?.id ?? null, notify);
      return;
    }
    selectionSynced = true;
  },
  { immediate: true },
);

watch(selectedItem, () => {
  pushedDetail.value = null;
  actionStatus.value = null;
});

async function executeAction(action: CommandSnapshotNode): Promise<void> {
  if (action.type === "Action.Push") {
    pushedDetail.value =
      action.children.find((child) => child.type === "Detail") ?? action.children[0] ?? null;
    actionStatus.value = "Открыто";
    return;
  }

  if (
    action.type === "Action" ||
    action.type === "Action.CopyToClipboard" ||
    action.type === "Action.Paste" ||
    action.type === "Action.Pop" ||
    action.type === "Action.PopToRoot" ||
    action.type === "Action.OpenInBrowser" ||
    action.type === "Action.Open" ||
    action.type === "Action.ShowInFinder" ||
    action.type === "Action.Trash" ||
    action.type === "Action.LaunchCommand"
  ) {
    const result = await window.kepler.command.action(props.sessionId, {
      type: action.type,
      props: action.props,
    });
    actionStatus.value = result.ok ? actionSuccessMessage(action.type) : "Действие не выполнено";
  }
}

async function executeDropdownChange(): Promise<void> {
  if (!dropdown.value?.node.props.__callbackId) return;
  const result = await window.kepler.command.action(props.sessionId, {
    type: dropdown.value.node.type,
    props: dropdown.value.node.props,
    payload: { value: dropdownValue.value },
  });
  actionStatus.value = result.ok ? "Выбрано" : "Выбор не применён";
}

async function updateSearch(event: Event): Promise<void> {
  const next = event.target instanceof HTMLInputElement ? event.target.value : "";
  query.value = next;
  actionStatus.value = null;

  const callbackNode = searchCallbackNode.value;
  const callbackId = callbackNode?.props.__onSearchTextChangeId;
  if (typeof callbackId !== "string") return;

  const result = await window.kepler.command.action(props.sessionId, {
    type: callbackNode.type,
    props: { __callbackId: callbackId },
    payload: { text: next },
  });
  actionStatus.value = result.ok ? "Поиск обновлён" : "Поиск не применён";
}

async function selectItem(id: string | null, notify = true): Promise<void> {
  if (selectedId.value === id) return;
  selectedId.value = id;
  actionStatus.value = null;

  if (!notify) return;
  const callbackNode = selectionCallbackNode.value;
  const callbackId = callbackNode?.props.__onSelectionChangeId;
  if (typeof callbackId !== "string") return;

  const result = await window.kepler.command.action(props.sessionId, {
    type: callbackNode.type,
    props: { __callbackId: callbackId },
    payload: { id },
  });
  actionStatus.value = result.ok ? "Выбрано" : "Выбор не применён";
}

function actionSuccessMessage(type: string): string {
  if (type === "Action") return "Готово";
  if (type === "Action.CopyToClipboard") return "Скопировано";
  if (type === "Action.Paste") return "Вставлено";
  if (type === "Action.Pop") return "Назад";
  if (type === "Action.PopToRoot") return "К началу";
  if (type === "Action.OpenInBrowser" || type === "Action.Open") return "Открыто";
  if (type === "Action.ShowInFinder") return "Показано";
  if (type === "Action.Trash") return "Удалено";
  if (type === "Action.LaunchCommand") return "Запущено";
  return "Готово";
}
</script>

<template>
  <section class="command-list-view" aria-label="Command List">
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
