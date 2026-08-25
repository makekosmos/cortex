import type { CommandSnapshotNode } from "../../shared/command-ipc";

type CommandPrimitive = string | number | boolean | null | undefined | Date;
export type CommandRecord = { [key: string]: CommandValue };
type CommandCallback = (...args: CommandValue[]) => void;
export type CommandValue = CommandPrimitive | CommandValue[] | CommandRecord | CommandCallback;

export interface CommandViewCallbackRegistry {
  register(callback: (payload?: CommandRecord) => void | Promise<void>): string;
}

const EMPTY_COMMAND_PROPS: CommandRecord = {};

export function isRecord(value: CommandValue): value is CommandRecord {
  return Object.prototype.toString.call(value) === "[object Object]";
}

function isCallback(value: CommandValue): value is CommandCallback {
  return Object.prototype.toString.call(value) === "[object Function]";
}

export function isString(value: CommandValue): value is string {
  return typeof value === "string";
}

function isNumber(value: CommandValue): value is number {
  return typeof value === "number";
}

function isBoolean(value: CommandValue): value is boolean {
  return typeof value === "boolean";
}

function isCommandElement(value: CommandValue): value is {
  type: string;
  props?: CommandRecord;
} {
  return (
    !!value &&
    isRecord(value) &&
    value.$$typeof === "kosmos.raycast.element" &&
    Object.prototype.toString.call(value.type) === "[object String]"
  );
}

function serializableProp(value: CommandValue): CommandValue | undefined {
  if (
    value === null ||
    isString(value) ||
    isNumber(value) ||
    isBoolean(value)
  ) {
    return value;
  }
  if (value instanceof Date) {
    return Number.isNaN(value.valueOf()) ? undefined : value.toISOString();
  }
  if (Array.isArray(value)) {
    return value
      .map(serializableProp)
      .filter((item): item is CommandValue => item !== undefined);
  }
  if (isRecord(value)) {
    const out: CommandRecord = {};
    for (const [key, item] of Object.entries(value)) {
      const serialized = serializableProp(item);
      if (serialized !== undefined) out[key] = serialized;
    }
    return out;
  }
  return;
}

export function normalizeCommandNode(
  value: CommandValue,
  callbacks?: CommandViewCallbackRegistry,
): CommandSnapshotNode | null {
  if (value === null || value === undefined || isBoolean(value)) return null;
  if (isString(value) || isNumber(value)) {
    return { type: "Text", text: String(value), props: {}, children: [] };
  }
  if (Array.isArray(value)) {
    return { type: "Fragment", props: {}, children: normalizeChildren(value, callbacks) };
  }
  if (!isCommandElement(value)) return null;

  const props = value.props ?? EMPTY_COMMAND_PROPS;
  const outProps: CommandRecord = {};
  for (const [key, item] of Object.entries(props)) {
    if (key === "children" || isCallback(item)) continue;
    if (
      key === "actions" ||
      key === "detail" ||
      key === "metadata" ||
      key === "searchBarAccessory"
    ) {
      continue;
    }
    if (key === "target" && value.type === "Action.Push") continue;
    const serialized = serializableProp(item);
    if (serialized !== undefined) outProps[key] = serialized;
  }
  const onAction = props.onAction;
  if (callbacks && isCallback(onAction)) {
    outProps.__callbackId = callbacks.register(() => {
      onAction();
    });
  }
  const onSubmit = props.onSubmit;
  if (callbacks && isCallback(onSubmit)) {
    outProps.__callbackId = callbacks.register((payload) => {
      onSubmit(payload?.values ?? {});
    });
  }
  const onChange = props.onChange;
  if (callbacks && isCallback(onChange)) {
    outProps.__callbackId = callbacks.register((payload) => {
      if (value.type === "Form.DatePicker") {
        onChange(datePickerCallbackValue(payload?.value));
        return;
      }
      onChange(payload?.value);
    });
  }
  const onSearchTextChange = props.onSearchTextChange;
  if (callbacks && isCallback(onSearchTextChange)) {
    outProps.__onSearchTextChangeId = callbacks.register((payload) =>
      onSearchTextChange(isString(payload?.text) ? payload.text : ""),
    );
  }
  const onSelectionChange = props.onSelectionChange;
  if (callbacks && isCallback(onSelectionChange)) {
    outProps.__onSelectionChangeId = callbacks.register((payload) =>
      onSelectionChange(isString(payload?.id) ? payload.id : null),
    );
  }
  const specialChildren =
    value.type === "List" || value.type === "Grid"
      ? normalizeChildren(props.searchBarAccessory, callbacks)
      : value.type === "List.Item" || value.type === "Grid.Item"
        ? normalizeChildren([props.detail, props.actions], callbacks)
        : value.type === "List.Item.Detail"
          ? normalizeChildren(props.metadata, callbacks)
          : value.type === "List.EmptyView" || value.type === "Grid.EmptyView"
            ? normalizeChildren(props.actions, callbacks)
            : value.type === "Detail"
              ? normalizeChildren([props.metadata, props.actions], callbacks)
              : value.type === "Form"
                ? normalizeChildren(props.actions, callbacks)
                : value.type === "Action.Push"
                  ? normalizeChildren(props.target, callbacks)
                  : [];
  return {
    type: value.type,
    props: outProps,
    children: [...normalizeChildren(props.children, callbacks), ...specialChildren],
  };
}

function datePickerCallbackValue(value: CommandValue): Date | null {
  if (value instanceof Date) return Number.isNaN(value.valueOf()) ? null : value;
  if (!isString(value) || value.trim().length === 0) return null;
  const date = /^\d{4}-\d{2}-\d{2}$/.test(value)
    ? new Date(`${value}T00:00:00.000Z`)
    : new Date(value);
  return Number.isNaN(date.valueOf()) ? null : date;
}

function normalizeChildren(
  value: CommandValue,
  callbacks?: CommandViewCallbackRegistry,
): CommandSnapshotNode[] {
  if (Array.isArray(value)) return value.flatMap((item) => normalizeChildren(item, callbacks));
  const node = normalizeCommandNode(value, callbacks);
  return node ? [node] : [];
}
