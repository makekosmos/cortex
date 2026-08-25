import { computed, shallowRef, watch } from "vue";
import type { CommandSnapshotNode } from "../../shared/command-ipc";
import { isString } from "../shared/runtimeGuards";
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

export function useCommandListView(props: { root: CommandSnapshotNode; sessionId: string }) {
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
    if (!callbackNode || !isString(callbackId)) return;

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
    if (!callbackNode || !isString(callbackId)) return;

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

  return {
    query,
    selectedId,
    dropdownValue,
    pushedDetail,
    actionStatus,
    items,
    sections,
    dropdown,
    filtering,
    searchCallbackNode,
    selectionCallbackNode,
    visibleSections,
    visibleItems,
    selectedItem,
    detail,
    placeholder,
    emptyMessage,
    emptyActions,
    isLoading,
    activeActions,
    executeAction,
    executeDropdownChange,
    updateSearch,
    selectItem,
    actionSuccessMessage,
  };
}
