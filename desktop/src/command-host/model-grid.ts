import type { CommandSnapshotNode } from "../../shared/command-ipc";
import { isString } from "../shared/runtimeGuards";
import {
  collectNodes,
  findFirst,
  imageProp,
  stringArrayProp,
  textProp,
  type CommandGridDropdownModel,
  type CommandGridItemModel,
  type CommandGridSectionModel,
  type CommandListDropdownOptionModel,
  type CommandListDropdownSectionModel,
} from "./model";

function gridItemModel(node: CommandSnapshotNode, index: number): CommandGridItemModel {
  const title = textProp(node.props.title) ?? "Без названия";
  const id = textProp(node.props.id) ?? `${index}:${title}`;
  return {
    id,
    title,
    subtitle: textProp(node.props.subtitle),
    keywords: stringArrayProp(node.props.keywords),
    image: imageProp(node.props.content) ?? imageProp(node.props.icon),
    actions: findFirst(node, "ActionPanel"),
  };
}

export function gridSections(snapshot: CommandSnapshotNode): CommandGridSectionModel[] {
  const sections: CommandGridSectionModel[] = [];
  let looseItems: CommandSnapshotNode[] = [];
  let itemIndex = 0;

  function flushLooseItems(): void {
    if (looseItems.length === 0) return;
    sections.push({
      id: `grid-section:root:${sections.length}`,
      title: null,
      items: looseItems.map((item) => gridItemModel(item, itemIndex++)),
    });
    looseItems = [];
  }

  for (const child of snapshot.children) {
    if (child.type === "Grid.Section") {
      flushLooseItems();
      const sectionItems = collectNodes(child, "Grid.Item").map((item) =>
        gridItemModel(item, itemIndex++),
      );
      sections.push({
        id: textProp(child.props.id) ?? `grid-section:${sections.length}`,
        title: textProp(child.props.title),
        items: sectionItems,
      });
    } else if (child.type === "Grid.Item") {
      looseItems.push(child);
    }
  }
  flushLooseItems();

  return sections;
}

export function gridItems(snapshot: CommandSnapshotNode): CommandGridItemModel[] {
  return gridSections(snapshot).flatMap((section) => section.items);
}

export function gridPlaceholder(snapshot: CommandSnapshotNode): string {
  return textProp(snapshot.props.searchBarPlaceholder) ?? "Поиск";
}

export function gridEmptyMessage(snapshot: CommandSnapshotNode): string {
  const emptyView = findFirst(snapshot, "Grid.EmptyView");
  return (
    textProp(emptyView?.props.title) ??
    textProp(emptyView?.props.description) ??
    "Ничего не найдено"
  );
}

export function gridEmptyActions(snapshot: CommandSnapshotNode): CommandSnapshotNode | null {
  const emptyView = findFirst(snapshot, "Grid.EmptyView");
  return emptyView ? findFirst(emptyView, "ActionPanel") : null;
}

export function gridIsLoading(snapshot: CommandSnapshotNode): boolean {
  return snapshot.props.isLoading === true;
}

export function gridFiltering(snapshot: CommandSnapshotNode): boolean {
  return snapshot.props.filtering !== false;
}

export function gridSearchText(snapshot: CommandSnapshotNode): string {
  return textProp(snapshot.props.searchText) ?? "";
}

export function gridSelectedItemId(snapshot: CommandSnapshotNode): string | null {
  return textProp(snapshot.props.selectedItemId);
}

export function gridSearchCallbackNode(snapshot: CommandSnapshotNode): CommandSnapshotNode | null {
  return isString(snapshot.props.__onSearchTextChangeId) ? snapshot : null;
}

export function gridSelectionCallbackNode(
  snapshot: CommandSnapshotNode,
): CommandSnapshotNode | null {
  return isString(snapshot.props.__onSelectionChangeId) ? snapshot : null;
}

export function gridDropdown(snapshot: CommandSnapshotNode): CommandGridDropdownModel | null {
  const dropdown = findFirst(snapshot, "Grid.Dropdown");
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
      id: `grid-dropdown-section:root:${sections.length}`,
      title: null,
      options: looseItems.map(optionModel),
    });
    looseItems = [];
  }

  for (const child of dropdown.children) {
    if (child.type === "Grid.Dropdown.Section") {
      flushLooseItems();
      sections.push({
        id: textProp(child.props.id) ?? `grid-dropdown-section:${sections.length}`,
        title: textProp(child.props.title),
        options: collectNodes(child, "Grid.Dropdown.Item").map(optionModel),
      });
    } else if (child.type === "Grid.Dropdown.Item") {
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

export function matchesGridItem(item: CommandGridItemModel, query: string): boolean {
  const normalized = query.trim().toLowerCase();
  if (!normalized) return true;
  const haystack = [item.title, item.subtitle ?? "", ...item.keywords].join(" ").toLowerCase();
  return haystack.includes(normalized);
}
