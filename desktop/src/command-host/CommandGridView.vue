<script setup lang="ts">
import { computed, shallowRef, watch } from "vue";
import type { CommandSnapshotNode } from "../../shared/command-ipc";
import CommandActionPanel from "./CommandActionPanel.vue";
import {
  gridDropdown,
  gridEmptyActions,
  gridEmptyMessage,
  gridFiltering,
  gridIsLoading,
  gridPlaceholder,
  gridSearchCallbackNode,
  gridSearchText,
  gridSections,
  gridSelectedItemId,
  gridSelectionCallbackNode,
  matchesGridItem,
} from "./model-grid";

const props = defineProps<{
  root: CommandSnapshotNode;
  sessionId: string;
}>();

const query = shallowRef("");
const selectedId = shallowRef<string | null>(null);
const dropdownValue = shallowRef("");
const actionStatus = shallowRef<string | null>(null);

const sections = computed(() => gridSections(props.root));
const dropdown = computed(() => gridDropdown(props.root));
const filtering = computed(() => gridFiltering(props.root));
const searchCallbackNode = computed(() => gridSearchCallbackNode(props.root));
const selectionCallbackNode = computed(() => gridSelectionCallbackNode(props.root));
const visibleSections = computed(() =>
  sections.value
    .map((section) => ({
      ...section,
      items: filtering.value
        ? section.items.filter((item) => matchesGridItem(item, query.value))
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
const placeholder = computed(() => gridPlaceholder(props.root));
const emptyMessage = computed(() => gridEmptyMessage(props.root));
const emptyActions = computed(() => gridEmptyActions(props.root));
const isLoading = computed(() => gridIsLoading(props.root));
const activeActions = computed(
  () =>
    selectedItem.value?.actions ??
    (visibleItems.value.length === 0 && !isLoading.value ? emptyActions.value : null),
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

watch(
  () => gridSearchText(props.root),
  (next) => {
    query.value = next;
  },
  { immediate: true },
);

watch(
  () => gridSelectedItemId(props.root),
  (next) => {
    selectedId.value = next;
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
  actionStatus.value = null;
});

async function executeAction(action: CommandSnapshotNode): Promise<void> {
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

function placeholderLetter(title: string): string {
  return title.trim().slice(0, 1).toUpperCase() || "?";
}
</script>

<template>
  <section class="command-grid-view" aria-label="Command Grid">
    <div class="command-grid-view__search">
      <input
        :value="query"
        class="command-grid-view__input"
        type="search"
        :placeholder="placeholder"
        aria-label="Поиск"
        @input="updateSearch"
      />
      <select
        v-if="dropdown"
        v-model="dropdownValue"
        class="command-grid-view__dropdown"
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

    <div class="command-grid-view__body kosmos-scroll">
      <p v-if="isLoading" class="command-grid-view__loading">Загрузка...</p>
      <section
        v-for="section in visibleSections"
        :key="section.id"
        class="command-grid-view__section"
      >
        <h2 v-if="section.title" class="command-grid-view__section-title">{{ section.title }}</h2>
        <div class="command-grid-view__items">
          <button
            v-for="item in section.items"
            :key="item.id"
            class="command-grid-view__item"
            :class="{ 'command-grid-view__item--selected': item.id === selectedItem?.id }"
            type="button"
            @click="selectItem(item.id)"
          >
            <span class="command-grid-view__preview">
              <img
                v-if="item.image"
                class="command-grid-view__image"
                :src="item.image"
                :alt="item.title"
              />
              <span v-else class="command-grid-view__placeholder">
                {{ placeholderLetter(item.title) }}
              </span>
            </span>
            <span class="command-grid-view__title">{{ item.title }}</span>
            <span v-if="item.subtitle" class="command-grid-view__subtitle">{{
              item.subtitle
            }}</span>
          </button>
        </div>
      </section>

      <p v-if="visibleItems.length === 0 && !isLoading" class="command-grid-view__empty">
        {{ emptyMessage }}
      </p>
    </div>

    <footer class="command-grid-view__footer">
      <CommandActionPanel :panel="activeActions" @execute="executeAction" />
      <p v-if="actionStatus" class="command-grid-view__status">{{ actionStatus }}</p>
    </footer>
  </section>
</template>

<style scoped>
.command-grid-view {
  display: flex;
  min-height: 0;
  min-width: 0;
  flex: 1;
  flex-direction: column;
  overflow: hidden;
}

.command-grid-view__search {
  display: flex;
  flex-shrink: 0;
  gap: 8px;
  border-bottom: 1px solid var(--border);
  padding: 10px 12px;
}

.command-grid-view__input {
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

.command-grid-view__dropdown {
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

.command-grid-view__input:focus,
.command-grid-view__dropdown:focus {
  border-color: var(--accent);
}

.command-grid-view__body {
  min-height: 0;
  flex: 1;
  overflow: auto;
  padding: 12px;
}

.command-grid-view__section + .command-grid-view__section {
  margin-top: 14px;
}

.command-grid-view__section-title {
  margin: 0 0 8px;
  color: var(--muted-foreground);
  font-size: 11px;
  font-weight: 700;
}

.command-grid-view__items {
  display: grid;
  grid-template-columns: repeat(auto-fill, minmax(132px, 1fr));
  gap: 10px;
}

.command-grid-view__item {
  display: grid;
  min-width: 0;
  gap: 6px;
  border: 1px solid transparent;
  border-radius: var(--radius-card);
  background: transparent;
  color: var(--foreground);
  padding: 8px;
  text-align: left;
}

.command-grid-view__item--selected {
  border-color: color-mix(in srgb, var(--accent) 42%, transparent);
  background: color-mix(in srgb, var(--accent) 18%, transparent);
}

.command-grid-view__preview {
  display: grid;
  aspect-ratio: 1;
  place-items: center;
  overflow: hidden;
  border: 1px solid var(--border);
  border-radius: var(--radius-card);
  background: color-mix(in srgb, var(--foreground) 5%, transparent);
}

.command-grid-view__image {
  width: 100%;
  height: 100%;
  object-fit: cover;
}

.command-grid-view__placeholder {
  color: var(--muted-foreground);
  font-size: 28px;
  font-weight: 700;
}

.command-grid-view__title,
.command-grid-view__subtitle {
  overflow: hidden;
  text-overflow: ellipsis;
  white-space: nowrap;
}

.command-grid-view__title {
  font-size: 13px;
  font-weight: 700;
}

.command-grid-view__subtitle,
.command-grid-view__empty,
.command-grid-view__loading,
.command-grid-view__status {
  color: var(--muted-foreground);
  font-size: 12px;
}

.command-grid-view__empty,
.command-grid-view__loading,
.command-grid-view__status {
  margin: 0;
}

.command-grid-view__loading {
  margin-bottom: 12px;
}

.command-grid-view__footer {
  display: flex;
  min-height: 44px;
  align-items: center;
  gap: 10px;
  border-top: 1px solid var(--border);
  padding: 8px 10px;
}
</style>
