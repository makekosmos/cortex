import type { RaycastSnapshotNode } from "../../shared/raycast-ipc";

export interface RaycastListItemModel {
  id: string;
  title: string;
  subtitle: string | null;
  icon: string | null;
  keywords: string[];
  accessories: string[];
  detail: RaycastSnapshotNode | null;
  actions: RaycastSnapshotNode | null;
}

export interface RaycastListSectionModel {
  id: string;
  title: string | null;
  items: RaycastListItemModel[];
}

export interface RaycastListDropdownOptionModel {
  value: string;
  title: string;
}

export interface RaycastListDropdownSectionModel {
  id: string;
  title: string | null;
  options: RaycastListDropdownOptionModel[];
}

export interface RaycastListDropdownModel {
  node: RaycastSnapshotNode;
  placeholder: string | null;
  defaultValue: string | null;
  sections: RaycastListDropdownSectionModel[];
  options: RaycastListDropdownOptionModel[];
}

export interface RaycastFormFieldModel {
  id: string;
  type: string;
  title: string;
  placeholder: string | null;
  defaultValue: string | boolean | string[] | null;
  callbackId: string | null;
  required: boolean;
  allowMultipleSelection: boolean;
  canChooseDirectories: boolean;
  canChooseFiles: boolean;
  showHiddenFiles: boolean;
  optionSections: RaycastListDropdownSectionModel[];
  options: { value: string; title: string }[];
}

export interface RaycastFormModel {
  fields: RaycastFormFieldModel[];
  actions: RaycastSnapshotNode | null;
}

export interface RaycastGridItemModel {
  id: string;
  title: string;
  subtitle: string | null;
  keywords: string[];
  image: string | null;
  actions: RaycastSnapshotNode | null;
}

export interface RaycastGridSectionModel {
  id: string;
  title: string | null;
  items: RaycastGridItemModel[];
}

export interface RaycastGridDropdownModel {
  node: RaycastSnapshotNode;
  placeholder: string | null;
  defaultValue: string | null;
  sections: RaycastListDropdownSectionModel[];
  options: RaycastListDropdownOptionModel[];
}

export type RaycastMetadataItemModel =
  | {
      id: string;
      type: "label" | "link";
      title: string;
      text: string;
      href: string | null;
    }
  | {
      id: string;
      type: "tags";
      title: string;
      tags: string[];
    }
  | {
      id: string;
      type: "separator";
    };

export interface RaycastMenuBarItemModel {
  id: string;
  title: string;
  subtitle: string | null;
  node: RaycastSnapshotNode;
  children: RaycastMenuBarItemModel[];
}

export interface RaycastMenuBarSectionModel {
  id: string;
  title: string | null;
  items: RaycastMenuBarItemModel[];
}

export interface RaycastActionSectionModel {
  id: string;
  title: string | null;
  actions: RaycastSnapshotNode[];
}

export interface RaycastActionShortcutModel {
  key: string;
  code: string | null;
  modifiers: {
    ctrl: boolean;
    shift: boolean;
    alt: boolean;
    meta: boolean;
  };
  label: string;
}

const ACTION_TYPES = new Set([
  "Action",
  "Action.CopyToClipboard",
  "Action.Paste",
  "Action.Push",
  "Action.Pop",
  "Action.PopToRoot",
  "Action.OpenInBrowser",
  "Action.Open",
  "Action.ShowInFinder",
  "Action.Trash",
  "Action.LaunchCommand",
  "Action.SubmitForm",
]);

const ACTION_PANEL_ENTRY_TYPES = new Set([...ACTION_TYPES, "ActionPanel.Submenu"]);

function textProp(value: unknown): string | null {
  return typeof value === "string" && value.trim().length > 0 ? value : null;
}

function stringArrayProp(value: unknown): string[] {
  return Array.isArray(value)
    ? value.filter((item): item is string => typeof item === "string")
    : [];
}

function imageProp(value: unknown): string | null {
  if (typeof value === "string" && value.trim().length > 0) return value;
  if (value && typeof value === "object") {
    const source = (value as { source?: unknown }).source;
    if (typeof source === "string" && source.trim().length > 0) return source;
  }
  return null;
}

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

function findFirst(node: RaycastSnapshotNode, type: string): RaycastSnapshotNode | null {
  if (node.type === type) return node;
  for (const child of node.children) {
    const found = findFirst(child, type);
    if (found) return found;
  }
  return null;
}

function collectNodes(node: RaycastSnapshotNode, type: string): RaycastSnapshotNode[] {
  const out: RaycastSnapshotNode[] = node.type === type ? [node] : [];
  for (const child of node.children) out.push(...collectNodes(child, type));
  return out;
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

export function listRoot(snapshot: RaycastSnapshotNode): RaycastSnapshotNode | null {
  return findFirst(snapshot, "List");
}

export function detailMarkdown(node: RaycastSnapshotNode | null): string {
  if (!node) return "";
  const markdown = textProp(node.props.markdown);
  if (markdown) return markdown;
  return node.children
    .filter((child) => child.type !== "Detail.Metadata" && child.type !== "ActionPanel")
    .map((child) => child.text ?? detailMarkdown(child))
    .join("\n");
}

export function detailMetadata(node: RaycastSnapshotNode | null): RaycastSnapshotNode | null {
  return node ? findFirst(node, "Detail.Metadata") : null;
}

export function detailActions(node: RaycastSnapshotNode | null): RaycastSnapshotNode | null {
  return node ? findFirst(node, "ActionPanel") : null;
}

export function detailMetadataItems(node: RaycastSnapshotNode | null): RaycastMetadataItemModel[] {
  const metadata = detailMetadata(node);
  if (!metadata) return [];
  const items: RaycastMetadataItemModel[] = [];
  let index = 0;

  for (const child of metadata.children) {
    const id = textProp(child.props.id) ?? `metadata:${index++}`;
    if (child.type === "Detail.Metadata.Separator") {
      items.push({ id, type: "separator" });
      continue;
    }
    if (child.type === "Detail.Metadata.TagList") {
      const tags = collectNodes(child, "Detail.Metadata.TagList.Item")
        .map((item) => textProp(item.props.text) ?? textProp(item.props.title))
        .filter((tag): tag is string => tag !== null);
      items.push({
        id,
        type: "tags",
        title: textProp(child.props.title) ?? "",
        tags,
      });
      continue;
    }
    if (child.type === "Detail.Metadata.Link") {
      items.push({
        id,
        type: "link",
        title: textProp(child.props.title) ?? "",
        text: textProp(child.props.text) ?? textProp(child.props.target) ?? "",
        href: textProp(child.props.target),
      });
      continue;
    }
    if (child.type === "Detail.Metadata.Label") {
      items.push({
        id,
        type: "label",
        title: textProp(child.props.title) ?? "",
        text: textProp(child.props.text) ?? "",
        href: null,
      });
    }
  }

  return items;
}

export function actionNodes(panel: RaycastSnapshotNode | null): RaycastSnapshotNode[] {
  return actionSections(panel).flatMap((section) => section.actions.flatMap(actionNodeLeaves));
}

export function actionSections(panel: RaycastSnapshotNode | null): RaycastActionSectionModel[] {
  if (!panel) return [];
  const sections: RaycastActionSectionModel[] = [];
  let looseActions: RaycastSnapshotNode[] = [];

  function flushLooseActions(): void {
    if (looseActions.length === 0) return;
    sections.push({
      id: `action-section:root:${sections.length}`,
      title: null,
      actions: looseActions,
    });
    looseActions = [];
  }

  for (const child of panel.children) {
    if (child.type === "ActionPanel.Section") {
      flushLooseActions();
      const actions = child.children.filter(isActionPanelEntry);
      if (actions.length > 0) {
        sections.push({
          id: textProp(child.props.id) ?? `action-section:${sections.length}`,
          title: textProp(child.props.title),
          actions,
        });
      }
    } else if (isActionPanelEntry(child)) {
      looseActions.push(child);
    }
  }
  flushLooseActions();

  return sections;
}

function isActionNode(node: RaycastSnapshotNode): boolean {
  return ACTION_TYPES.has(node.type);
}

function isActionPanelEntry(node: RaycastSnapshotNode): boolean {
  return ACTION_PANEL_ENTRY_TYPES.has(node.type);
}

function actionNodeLeaves(node: RaycastSnapshotNode): RaycastSnapshotNode[] {
  if (node.type === "ActionPanel.Submenu") return actionSubmenuActions(node);
  return isActionNode(node) ? [node] : [];
}

export function actionSubmenuActions(node: RaycastSnapshotNode): RaycastSnapshotNode[] {
  return node.children.flatMap(actionNodeLeaves);
}

export function actionShortcut(action: RaycastSnapshotNode): RaycastActionShortcutModel | null {
  const shortcut = action.props.shortcut;
  if (!shortcut || typeof shortcut !== "object") return null;
  const record = shortcut as Record<string, unknown>;
  const key = textProp(record.key);
  if (!key) return null;

  const modifiers = stringArrayProp(record.modifiers).map((item) => item.toLowerCase());
  const normalized = {
    ctrl: modifiers.some((item) => item === "ctrl" || item === "control"),
    shift: modifiers.includes("shift"),
    alt: modifiers.some((item) => item === "alt" || item === "option"),
    meta: modifiers.some((item) => item === "cmd" || item === "command" || item === "meta"),
  };
  const normalizedKey = key.toLowerCase();
  const code = /^[a-z]$/.test(normalizedKey) ? `Key${normalizedKey.toUpperCase()}` : null;
  const parts = [
    normalized.meta ? "Cmd" : null,
    normalized.ctrl ? "Ctrl" : null,
    normalized.alt ? "Alt" : null,
    normalized.shift ? "Shift" : null,
    shortcutKeyLabel(key),
  ].filter((item): item is string => item !== null);

  return {
    key: normalizedKey,
    code,
    modifiers: normalized,
    label: parts.join(" "),
  };
}

export function matchesActionShortcut(
  shortcut: RaycastActionShortcutModel | null,
  event: Pick<KeyboardEvent, "altKey" | "code" | "ctrlKey" | "key" | "metaKey" | "shiftKey">,
): boolean {
  if (!shortcut) return false;
  if (event.ctrlKey !== shortcut.modifiers.ctrl) return false;
  if (event.shiftKey !== shortcut.modifiers.shift) return false;
  if (event.altKey !== shortcut.modifiers.alt) return false;
  if (event.metaKey !== shortcut.modifiers.meta) return false;
  if (shortcut.code) return event.code === shortcut.code;
  return event.key.toLowerCase() === shortcut.key;
}

function shortcutKeyLabel(key: string): string {
  if (key.length === 1) return key.toUpperCase();
  if (key.toLowerCase() === "enter") return "Enter";
  if (key.toLowerCase() === "return") return "Enter";
  if (key.toLowerCase() === "escape") return "Esc";
  if (key.toLowerCase() === "backspace") return "Backspace";
  if (key.toLowerCase() === "delete") return "Delete";
  if (key.toLowerCase() === "space") return "Space";
  return key;
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

export function menuBarTitle(snapshot: RaycastSnapshotNode): string {
  return textProp(snapshot.props.title) ?? textProp(snapshot.props.tooltip) ?? "Menu Bar";
}

export function menuBarIsLoading(snapshot: RaycastSnapshotNode): boolean {
  return snapshot.props.isLoading === true;
}

export function menuBarSections(snapshot: RaycastSnapshotNode): RaycastMenuBarSectionModel[] {
  const sections: RaycastMenuBarSectionModel[] = [];
  let looseItems: RaycastSnapshotNode[] = [];
  let itemIndex = 0;

  function menuItemModel(node: RaycastSnapshotNode): RaycastMenuBarItemModel {
    const title = textProp(node.props.title) ?? "Без названия";
    const id = textProp(node.props.id) ?? `${itemIndex}:${title}`;
    itemIndex += 1;
    return {
      id,
      title,
      subtitle: textProp(node.props.subtitle),
      node,
      children: node.children
        .filter(
          (child) => child.type === "MenuBarExtra.Item" || child.type === "MenuBarExtra.Submenu",
        )
        .map(menuItemModel),
    };
  }

  function flushLooseItems(): void {
    if (looseItems.length === 0) return;
    sections.push({
      id: `menubar-section:root:${sections.length}`,
      title: null,
      items: looseItems.map(menuItemModel),
    });
    looseItems = [];
  }

  for (const child of snapshot.children) {
    if (child.type === "MenuBarExtra.Section") {
      flushLooseItems();
      sections.push({
        id: textProp(child.props.id) ?? `menubar-section:${sections.length}`,
        title: textProp(child.props.title),
        items: child.children
          .filter(
            (item) => item.type === "MenuBarExtra.Item" || item.type === "MenuBarExtra.Submenu",
          )
          .map(menuItemModel),
      });
    } else if (child.type === "MenuBarExtra.Item" || child.type === "MenuBarExtra.Submenu") {
      looseItems.push(child);
    }
  }
  flushLooseItems();

  return sections;
}

export function formModel(snapshot: RaycastSnapshotNode): RaycastFormModel {
  const fields: RaycastFormFieldModel[] = [];
  let index = 0;
  for (const child of snapshot.children) {
    if (!child.type.startsWith("Form.")) continue;
    if (
      child.type === "Form.Dropdown.Item" ||
      child.type === "Form.Dropdown.Section" ||
      child.type === "Form.TagPicker.Item"
    ) {
      continue;
    }
    const id = textProp(child.props.id) ?? textProp(child.props.name) ?? `${index}:${child.type}`;
    const optionSections =
      child.type === "Form.Dropdown"
        ? formDropdownSections(child)
        : child.type === "Form.TagPicker"
          ? formTagPickerSections(child)
          : [];
    fields.push({
      id,
      type: child.type,
      title: textProp(child.props.title) ?? textProp(child.props.text) ?? "Без названия",
      placeholder: textProp(child.props.placeholder),
      defaultValue: formDefaultValue(child),
      callbackId: textProp(child.props.__callbackId),
      required: child.props.storeValue === true || child.props.required === true,
      allowMultipleSelection: child.props.allowMultipleSelection === true,
      canChooseDirectories: child.props.canChooseDirectories === true,
      canChooseFiles: child.props.canChooseFiles !== false,
      showHiddenFiles: child.props.showHiddenFiles === true,
      optionSections,
      options: optionSections.flatMap((section) => section.options),
    });
    index += 1;
  }
  return {
    fields,
    actions: findFirst(snapshot, "ActionPanel"),
  };
}

function formDropdownSections(node: RaycastSnapshotNode): RaycastListDropdownSectionModel[] {
  const sections: RaycastListDropdownSectionModel[] = [];
  let looseItems: RaycastSnapshotNode[] = [];
  let optionIndex = 0;

  function optionModel(item: RaycastSnapshotNode): RaycastListDropdownOptionModel {
    const value = textProp(item.props.value) ?? textProp(item.props.id) ?? `${optionIndex}`;
    optionIndex += 1;
    return {
      value,
      title: textProp(item.props.title) ?? value,
    };
  }

  function flushLooseItems(): void {
    if (looseItems.length === 0) return;
    sections.push({
      id: `form-dropdown-section:root:${sections.length}`,
      title: null,
      options: looseItems.map(optionModel),
    });
    looseItems = [];
  }

  for (const child of node.children) {
    if (child.type === "Form.Dropdown.Section") {
      flushLooseItems();
      sections.push({
        id: textProp(child.props.id) ?? `form-dropdown-section:${sections.length}`,
        title: textProp(child.props.title),
        options: child.children
          .filter((item) => item.type === "Form.Dropdown.Item")
          .map(optionModel),
      });
    } else if (child.type === "Form.Dropdown.Item") {
      looseItems.push(child);
    }
  }
  flushLooseItems();

  return sections;
}

function formTagPickerSections(node: RaycastSnapshotNode): RaycastListDropdownSectionModel[] {
  const options = node.children
    .filter((item) => item.type === "Form.TagPicker.Item")
    .map((item, index): RaycastListDropdownOptionModel => {
      const value = textProp(item.props.value) ?? textProp(item.props.id) ?? `${index}`;
      return {
        value,
        title: textProp(item.props.title) ?? value,
      };
    });

  return options.length > 0
    ? [
        {
          id: "form-tag-picker-section:root:0",
          title: null,
          options,
        },
      ]
    : [];
}

function formDefaultValue(node: RaycastSnapshotNode): string | boolean | string[] | null {
  const value = node.props.defaultValue ?? node.props.value;
  if (node.type === "Form.FilePicker" || node.type === "Form.TagPicker") {
    if (Array.isArray(value))
      return value.filter((item): item is string => typeof item === "string");
    return typeof value === "string" ? [value] : [];
  }
  if (node.type === "Form.DatePicker") {
    return dateInputValue(value);
  }
  if (typeof value === "string" || typeof value === "boolean") return value;
  if (Array.isArray(value)) return value.filter((item): item is string => typeof item === "string");
  return null;
}

function dateInputValue(value: unknown): string | null {
  if (!(typeof value === "string" && value.trim().length > 0)) return null;
  const trimmed = value.trim();
  if (/^\d{4}-\d{2}-\d{2}$/.test(trimmed)) return trimmed;
  const date = new Date(trimmed);
  if (Number.isNaN(date.valueOf())) return null;
  return date.toISOString().slice(0, 10);
}
