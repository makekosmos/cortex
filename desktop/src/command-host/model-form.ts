import type { CommandSnapshotNode } from "../../shared/command-ipc";
import {
  findFirst,
  textProp,
  type CommandFormFieldModel,
  type CommandFormModel,
  type CommandListDropdownOptionModel,
  type CommandListDropdownSectionModel,
} from "./model";

export function formModel(snapshot: CommandSnapshotNode): CommandFormModel {
  const fields: CommandFormFieldModel[] = [];
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

function formDropdownSections(node: CommandSnapshotNode): CommandListDropdownSectionModel[] {
  const sections: CommandListDropdownSectionModel[] = [];
  let looseItems: CommandSnapshotNode[] = [];
  let optionIndex = 0;

  function optionModel(item: CommandSnapshotNode): CommandListDropdownOptionModel {
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

function formTagPickerSections(node: CommandSnapshotNode): CommandListDropdownSectionModel[] {
  const options = node.children
    .filter((item) => item.type === "Form.TagPicker.Item")
    .map((item, index): CommandListDropdownOptionModel => {
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

function formDefaultValue(node: CommandSnapshotNode): string | boolean | string[] | null {
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
