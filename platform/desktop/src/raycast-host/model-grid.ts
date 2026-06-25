import type { RaycastSnapshotNode } from "../../shared/raycast-ipc";
import {
  collectNodes,
  findFirst,
  imageProp,
  stringArrayProp,
  textProp,
  type RaycastGridDropdownModel,
  type RaycastGridItemModel,
  type RaycastGridSectionModel,
  type RaycastListDropdownOptionModel,
  type RaycastListDropdownSectionModel,
} from "./model";

function gridItemModel(node: RaycastSnapshotNode, index: number): RaycastGridItemModel {
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

export function gridSections(snapshot: RaycastSnapshotNode): RaycastGridSectionModel[] {
  const sections: RaycastGridSectionModel[] = [];
  let looseItems: RaycastSnapshotNode[] = [];
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

export function gridItems(snapshot: RaycastSnapshotNode): RaycastGridItemModel[] {
  return gridSections(snapshot).flatMap((section) => section.items);
}

export function gridPlaceholder(snapshot: RaycastSnapshotNode): string {
  return textProp(snapshot.props.searchBarPlaceholder) ?? "Поиск";
}

export function gridEmptyMessage(snapshot: RaycastSnapshotNode): string {
  const emptyView = findFirst(snapshot, "Grid.EmptyView");
  return (
    textProp(emptyView?.props.title) ??
    textProp(emptyView?.props.description) ??
    "Ничего не найдено"
  );
}

export function gridEmptyActions(snapshot: RaycastSnapshotNode): RaycastSnapshotNode | null {
  const emptyView = findFirst(snapshot, "Grid.EmptyView");
  return emptyView ? findFirst(emptyView, "ActionPanel") : null;
}

export function gridIsLoading(snapshot: RaycastSnapshotNode): boolean {
  return snapshot.props.isLoading === true;
}

export function gridFiltering(snapshot: RaycastSnapshotNode): boolean {
  return snapshot.props.filtering !== false;
}

export function gridSearchText(snapshot: RaycastSnapshotNode): string {
  return textProp(snapshot.props.searchText) ?? "";
}

export function gridSelectedItemId(snapshot: RaycastSnapshotNode): string | null {
  return textProp(snapshot.props.selectedItemId);
}

export function gridSearchCallbackNode(snapshot: RaycastSnapshotNode): RaycastSnapshotNode | null {
  return typeof snapshot.props.__onSearchTextChangeId === "string" ? snapshot : null;
}

export function gridSelectionCallbackNode(
  snapshot: RaycastSnapshotNode,
): RaycastSnapshotNode | null {
  return typeof snapshot.props.__onSelectionChangeId === "string" ? snapshot : null;
}

export function gridDropdown(snapshot: RaycastSnapshotNode): RaycastGridDropdownModel | null {
  const dropdown = findFirst(snapshot, "Grid.Dropdown");
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

export function matchesGridItem(item: RaycastGridItemModel, query: string): boolean {
  const normalized = query.trim().toLowerCase();
  if (!normalized) return true;
  const haystack = [item.title, item.subtitle ?? "", ...item.keywords].join(" ").toLowerCase();
  return haystack.includes(normalized);
}
