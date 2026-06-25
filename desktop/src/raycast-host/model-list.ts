import type { RaycastSnapshotNode } from "../../shared/raycast-ipc";
import { collectNodes, findFirst, imageProp, stringArrayProp, textProp } from "./model";
import type {
  RaycastListDropdownModel,
  RaycastListDropdownOptionModel,
  RaycastListDropdownSectionModel,
  RaycastListItemModel,
  RaycastListSectionModel,
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

function itemModel(node: RaycastSnapshotNode, index: number): RaycastListItemModel {
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

function listRoot(snapshot: RaycastSnapshotNode): RaycastSnapshotNode | null {
  return findFirst(snapshot, "List");
}

export function listItems(snapshot: RaycastSnapshotNode): RaycastListItemModel[] {
  return listSections(snapshot).flatMap((section) => section.items);
}

export function listSections(snapshot: RaycastSnapshotNode): RaycastListSectionModel[] {
  const root = listRoot(snapshot);
  if (!root) return [];
  const sections: RaycastListSectionModel[] = [];
  let looseItems: RaycastSnapshotNode[] = [];
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

export function listPlaceholder(snapshot: RaycastSnapshotNode): string {
  const root = listRoot(snapshot);
  return textProp(root?.props.searchBarPlaceholder) ?? "Поиск";
}

export function listEmptyMessage(snapshot: RaycastSnapshotNode): string {
  const root = listRoot(snapshot);
  const emptyView = root ? findFirst(root, "List.EmptyView") : null;
  return (
    textProp(emptyView?.props.title) ??
    textProp(emptyView?.props.description) ??
    "Ничего не найдено"
  );
}

export function listEmptyActions(snapshot: RaycastSnapshotNode): RaycastSnapshotNode | null {
  const root = listRoot(snapshot);
  const emptyView = root ? findFirst(root, "List.EmptyView") : null;
  return emptyView ? findFirst(emptyView, "ActionPanel") : null;
}

export function listIsLoading(snapshot: RaycastSnapshotNode): boolean {
  return listRoot(snapshot)?.props.isLoading === true;
}

export function listFiltering(snapshot: RaycastSnapshotNode): boolean {
  return listRoot(snapshot)?.props.filtering !== false;
}

export function listSearchText(snapshot: RaycastSnapshotNode): string {
  return textProp(listRoot(snapshot)?.props.searchText) ?? "";
}

export function listSelectedItemId(snapshot: RaycastSnapshotNode): string | null {
  return textProp(listRoot(snapshot)?.props.selectedItemId);
}

export function listSearchCallbackNode(snapshot: RaycastSnapshotNode): RaycastSnapshotNode | null {
  const root = listRoot(snapshot);
  return typeof root?.props.__onSearchTextChangeId === "string" ? root : null;
}

export function listSelectionCallbackNode(
  snapshot: RaycastSnapshotNode,
): RaycastSnapshotNode | null {
  const root = listRoot(snapshot);
  return typeof root?.props.__onSelectionChangeId === "string" ? root : null;
}

export function listDropdown(snapshot: RaycastSnapshotNode): RaycastListDropdownModel | null {
  const root = listRoot(snapshot);
  const dropdown = root ? findFirst(root, "List.Dropdown") : null;
  if (!dropdown) return null;

  const sections: RaycastListDropdownSectionModel[] = [];
  let looseItems: RaycastSnapshotNode[] = [];
  let optionIndex = 0;

  function optionModel(node: RaycastSnapshotNode): RaycastListDropdownOptionModel {
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

export function matchesItem(item: RaycastListItemModel, query: string): boolean {
  const normalized = query.trim().toLowerCase();
  if (!normalized) return true;
  const haystack = [item.title, item.subtitle ?? "", ...item.keywords].join(" ").toLowerCase();
  return haystack.includes(normalized);
}
