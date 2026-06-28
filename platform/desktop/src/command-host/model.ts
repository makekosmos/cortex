import type { CommandSnapshotNode } from "../../shared/command-ipc";

export interface CommandListItemModel {
  id: string;
  title: string;
  subtitle: string | null;
  icon: string | null;
  keywords: string[];
  accessories: string[];
  detail: CommandSnapshotNode | null;
  actions: CommandSnapshotNode | null;
}

export interface CommandListSectionModel {
  id: string;
  title: string | null;
  items: CommandListItemModel[];
}

export interface CommandListDropdownOptionModel {
  value: string;
  title: string;
}

export interface CommandListDropdownSectionModel {
  id: string;
  title: string | null;
  options: CommandListDropdownOptionModel[];
}

export interface CommandListDropdownModel {
  node: CommandSnapshotNode;
  placeholder: string | null;
  defaultValue: string | null;
  sections: CommandListDropdownSectionModel[];
  options: CommandListDropdownOptionModel[];
}

export interface CommandFormFieldModel {
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
  optionSections: CommandListDropdownSectionModel[];
  options: { value: string; title: string }[];
}

export interface CommandFormModel {
  fields: CommandFormFieldModel[];
  actions: CommandSnapshotNode | null;
}

export interface CommandGridItemModel {
  id: string;
  title: string;
  subtitle: string | null;
  keywords: string[];
  image: string | null;
  actions: CommandSnapshotNode | null;
}

export interface CommandGridSectionModel {
  id: string;
  title: string | null;
  items: CommandGridItemModel[];
}

export interface CommandGridDropdownModel {
  node: CommandSnapshotNode;
  placeholder: string | null;
  defaultValue: string | null;
  sections: CommandListDropdownSectionModel[];
  options: CommandListDropdownOptionModel[];
}

export type CommandMetadataItemModel =
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

export interface CommandMenuBarItemModel {
  id: string;
  title: string;
  subtitle: string | null;
  node: CommandSnapshotNode;
  children: CommandMenuBarItemModel[];
}

export interface CommandMenuBarSectionModel {
  id: string;
  title: string | null;
  items: CommandMenuBarItemModel[];
}

export interface CommandActionSectionModel {
  id: string;
  title: string | null;
  actions: CommandSnapshotNode[];
}

export interface CommandActionShortcutModel {
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

export function textProp(value: unknown): string | null {
  return typeof value === "string" && value.trim().length > 0 ? value : null;
}

export function stringArrayProp(value: unknown): string[] {
  return Array.isArray(value)
    ? value.filter((item): item is string => typeof item === "string")
    : [];
}

export function imageProp(value: unknown): string | null {
  if (typeof value === "string" && value.trim().length > 0) return value;
  if (value && typeof value === "object") {
    const source = (value as { source?: unknown }).source;
    if (typeof source === "string" && source.trim().length > 0) return source;
  }
  return null;
}

export function findFirst(node: CommandSnapshotNode, type: string): CommandSnapshotNode | null {
  if (node.type === type) return node;
  for (const child of node.children) {
    const found = findFirst(child, type);
    if (found) return found;
  }
  return null;
}

export function collectNodes(node: CommandSnapshotNode, type: string): CommandSnapshotNode[] {
  const out: CommandSnapshotNode[] = node.type === type ? [node] : [];
  for (const child of node.children) out.push(...collectNodes(child, type));
  return out;
}

export function actionNodes(panel: CommandSnapshotNode | null): CommandSnapshotNode[] {
  return actionSections(panel).flatMap((section) => section.actions.flatMap(actionNodeLeaves));
}

export function actionSections(panel: CommandSnapshotNode | null): CommandActionSectionModel[] {
  if (!panel) return [];
  const sections: CommandActionSectionModel[] = [];
  let looseActions: CommandSnapshotNode[] = [];

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

function isActionNode(node: CommandSnapshotNode): boolean {
  return ACTION_TYPES.has(node.type);
}

function isActionPanelEntry(node: CommandSnapshotNode): boolean {
  return ACTION_PANEL_ENTRY_TYPES.has(node.type);
}

function actionNodeLeaves(node: CommandSnapshotNode): CommandSnapshotNode[] {
  if (node.type === "ActionPanel.Submenu") return actionSubmenuActions(node);
  return isActionNode(node) ? [node] : [];
}

export function actionSubmenuActions(node: CommandSnapshotNode): CommandSnapshotNode[] {
  return node.children.flatMap(actionNodeLeaves);
}

export function actionShortcut(action: CommandSnapshotNode): CommandActionShortcutModel | null {
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
  shortcut: CommandActionShortcutModel | null,
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
