import type { CommandSnapshotNode } from "../../shared/command-ipc";

export interface CommandViewCallbackRegistry {
  register(callback: (payload?: Record<string, unknown>) => unknown | Promise<unknown>): string;
}

const EMPTY_COMMAND_PROPS: Record<string, unknown> = {};

function isCommandElement(value: unknown): value is {
  type: string;
  props?: Record<string, unknown>;
} {
  return (
    !!value &&
    typeof value === "object" &&
    (value as { $$typeof?: unknown }).$$typeof === "kosmos.raycast.element" &&
    typeof (value as { type?: unknown }).type === "string"
  );
}

function serializableProp(value: unknown): unknown {
  if (
    value === null ||
    typeof value === "string" ||
    typeof value === "number" ||
    typeof value === "boolean"
  ) {
    return value;
  }
  if (value instanceof Date) {
    return Number.isNaN(value.valueOf()) ? undefined : value.toISOString();
  }
  if (Array.isArray(value)) {
    return value
      .map(serializableProp)
      .filter((item): item is Exclude<unknown, undefined> => item !== undefined);
  }
  if (typeof value === "object") {
    const out: Record<string, unknown> = {};
    for (const [key, item] of Object.entries(value as Record<string, unknown>)) {
      const serialized = serializableProp(item);
      if (serialized !== undefined) out[key] = serialized;
    }
    return out;
  }
  return;
}

export function normalizeCommandNode(
  value: unknown,
  callbacks?: CommandViewCallbackRegistry,
): CommandSnapshotNode | null {
  if (value === null || value === undefined || typeof value === "boolean") return null;
  if (typeof value === "string" || typeof value === "number") {
    return { type: "Text", text: String(value), props: {}, children: [] };
  }
  if (Array.isArray(value)) {
    return { type: "Fragment", props: {}, children: normalizeChildren(value, callbacks) };
  }
  if (!isCommandElement(value)) return null;

  const props = value.props ?? EMPTY_COMMAND_PROPS;
  const outProps: Record<string, unknown> = {};
  for (const [key, item] of Object.entries(props)) {
    if (key === "children" || typeof item === "function") continue;
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
  if (callbacks && typeof onAction === "function") {
    outProps.__callbackId = callbacks.register(() => onAction());
  }
  const onSubmit = props.onSubmit;
  if (callbacks && typeof onSubmit === "function") {
    outProps.__callbackId = callbacks.register((payload) =>
      onSubmit((payload?.values ?? {}) as Record<string, unknown>),
    );
  }
  const onChange = props.onChange;
  if (callbacks && typeof onChange === "function") {
    outProps.__callbackId = callbacks.register((payload) => {
      if (value.type === "Form.DatePicker") {
        onChange(datePickerCallbackValue(payload?.value));
        return;
      }
      onChange(payload?.value);
    });
  }
  const onSearchTextChange = props.onSearchTextChange;
  if (callbacks && typeof onSearchTextChange === "function") {
    outProps.__onSearchTextChangeId = callbacks.register((payload) =>
      onSearchTextChange(typeof payload?.text === "string" ? payload.text : ""),
    );
  }
  const onSelectionChange = props.onSelectionChange;
  if (callbacks && typeof onSelectionChange === "function") {
    outProps.__onSelectionChangeId = callbacks.register((payload) =>
      onSelectionChange(typeof payload?.id === "string" ? payload.id : null),
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

function datePickerCallbackValue(value: unknown): Date | null {
  if (value instanceof Date) return Number.isNaN(value.valueOf()) ? null : value;
  if (typeof value !== "string" || value.trim().length === 0) return null;
  const date = /^\d{4}-\d{2}-\d{2}$/.test(value)
    ? new Date(`${value}T00:00:00.000Z`)
    : new Date(value);
  return Number.isNaN(date.valueOf()) ? null : date;
}

function normalizeChildren(
  value: unknown,
  callbacks?: CommandViewCallbackRegistry,
): CommandSnapshotNode[] {
  if (Array.isArray(value)) return value.flatMap((item) => normalizeChildren(item, callbacks));
  const node = normalizeCommandNode(value, callbacks);
  return node ? [node] : [];
}
