import type { CommandSnapshotNode } from "../../shared/command-ipc";
import { collectNodes, findFirst, imageProp, stringArrayProp, textProp } from "./model";
import type {
  CommandListDropdownModel,
  CommandListDropdownOptionModel,
  CommandListDropdownSectionModel,
  CommandListItemModel,
  CommandListSectionModel,
} from "./model";

function accessoryText(value: unknown): string | null {
  if (typeof value === "string" && value.trim().length > 0) return value;
  if (!value || typeof value !== "object") return null;
  const record = value as Record<string, unknown>;
  return (
    textProp(record.text) ?? textProp(record.title) ?? textProp(record.tag) ?? textProp(record.date)
  );
}

function accessoryTexts(value: unknown): string[] {
  if (!Array.isArray(value)) return [];
  return value.map(accessoryText).filter((item): item is string => item !== null);
}

function itemModel(node: CommandSnapshotNode, index: number): CommandListItemModel {
  const title = textProp(node.props.title) ?? "Без названия";
  const id = textProp(node.props.id) ?? `${index}:${title}`;
  return {
    id,
    title,
    subtitle: textProp(node.props.subtitle),
    icon: imageProp(node.props.icon),
    keywords: stringArrayProp(node.props.keywords),
    accessories: accessoryTexts(node.props.accessories),
    detail: findFirst(node, "List.Item.Detail") ?? findFirst(node, "Detail"),
    actions: findFirst(node, "ActionPanel"),
  };
}

function listRoot(snapshot: CommandSnapshotNode): CommandSnapshotNode | null {
  return findFirst(snapshot, "List");
}

export function listItems(snapshot: CommandSnapshotNode): CommandListItemModel[] {
  return listSections(snapshot).flatMap((section) => section.items);
}

export function listSections(snapshot: CommandSnapshotNode): CommandListSectionModel[] {
  const root = listRoot(snapshot);
  if (!root) return [];
  const sections: CommandListSectionModel[] = [];
  let looseItems: CommandSnapshotNode[] = [];
  let itemIndex = 0;

  function flushLooseItems(): void {
    if (looseItems.length === 0) return;
    sections.push({
      id: `section:root:${sections.length}`,
      title: null,
      items: looseItems.map((item) => itemModel(item, itemIndex++)),
    });
    looseItems = [];
  }

  for (const child of root.children) {
    if (child.type === "List.Section") {
      flushLooseItems();
      const sectionItems = collectNodes(child, "List.Item").map((item) =>
        itemModel(item, itemIndex++),
      );
      sections.push({
        id: textProp(child.props.id) ?? `section:${sections.length}`,
        title: textProp(child.props.title),
        items: sectionItems,
      });
    } else if (child.type === "List.Item") {
      looseItems.push(child);
    }
  }
  flushLooseItems();

  return sections;
}

export function listPlaceholder(snapshot: CommandSnapshotNode): string {
  const root = listRoot(snapshot);
  return textProp(root?.props.searchBarPlaceholder) ?? "Поиск";
}

export function listEmptyMessage(snapshot: CommandSnapshotNode): string {
  const root = listRoot(snapshot);
  const emptyView = root ? findFirst(root, "List.EmptyView") : null;
  return (
    textProp(emptyView?.props.title) ??
    textProp(emptyView?.props.description) ??
    "Ничего не найдено"
  );
}

export function listEmptyActions(snapshot: CommandSnapshotNode): CommandSnapshotNode | null {
  const root = listRoot(snapshot);
  const emptyView = root ? findFirst(root, "List.EmptyView") : null;
  return emptyView ? findFirst(emptyView, "ActionPanel") : null;
}

export function listIsLoading(snapshot: CommandSnapshotNode): boolean {
  return listRoot(snapshot)?.props.isLoading === true;
}

export function listFiltering(snapshot: CommandSnapshotNode): boolean {
  return listRoot(snapshot)?.props.filtering !== false;
}

export function listSearchText(snapshot: CommandSnapshotNode): string {
  return textProp(listRoot(snapshot)?.props.searchText) ?? "";
}

export function listSelectedItemId(snapshot: CommandSnapshotNode): string | null {
  return textProp(listRoot(snapshot)?.props.selectedItemId);
}

export function listSearchCallbackNode(snapshot: CommandSnapshotNode): CommandSnapshotNode | null {
  const root = listRoot(snapshot);
  return typeof root?.props.__onSearchTextChangeId === "string" ? root : null;
}

export function listSelectionCallbackNode(
  snapshot: CommandSnapshotNode,
): CommandSnapshotNode | null {
  const root = listRoot(snapshot);
  return typeof root?.props.__onSelectionChangeId === "string" ? root : null;
}

export function listDropdown(snapshot: CommandSnapshotNode): CommandListDropdownModel | null {
  const root = listRoot(snapshot);
  const dropdown = root ? findFirst(root, "List.Dropdown") : null;
  if (!dropdown) return null;

  const sections: CommandListDropdownSectionModel[] = [];
  let looseItems: CommandSnapshotNode[] = [];
  let optionIndex = 0;

  function optionModel(node: CommandSnapshotNode): CommandListDropdownOptionModel {
    const value = textProp(node.props.value) ?? textProp(node.props.id) ?? `${optionIndex}`;
    optionIndex += 1;
    return {
      value,
      title: textProp(node.props.title) ?? value,
    };
  }

  function flushLooseItems(): void {
    if (looseItems.length === 0) return;
    sections.push({
      id: `dropdown-section:root:${sections.length}`,
      title: null,
      options: looseItems.map(optionModel),
    });
    looseItems = [];
  }

  for (const child of dropdown.children) {
    if (child.type === "List.Dropdown.Section") {
      flushLooseItems();
      sections.push({
        id: textProp(child.props.id) ?? `dropdown-section:${sections.length}`,
        title: textProp(child.props.title),
        options: collectNodes(child, "List.Dropdown.Item").map(optionModel),
      });
    } else if (child.type === "List.Dropdown.Item") {
      looseItems.push(child);
    }
  }
  flushLooseItems();

  return {
    node: dropdown,
    placeholder: textProp(dropdown.props.placeholder) ?? textProp(dropdown.props.tooltip),
    defaultValue: textProp(dropdown.props.defaultValue) ?? textProp(dropdown.props.value),
    sections,
    options: sections.flatMap((section) => section.options),
  };
}

export function matchesItem(item: CommandListItemModel, query: string): boolean {
  const normalized = query.trim().toLowerCase();
  if (!normalized) return true;
  const haystack = [item.title, item.subtitle ?? "", ...item.keywords].join(" ").toLowerCase();
  return haystack.includes(normalized);
}
