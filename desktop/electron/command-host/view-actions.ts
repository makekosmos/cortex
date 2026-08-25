import { clipboard, shell } from "electron";
import type { LaunchCommandOptions } from "@raycast/api";
import type { CommandActionRequest, CommandActionResult } from "../../shared/command-ipc";
import type { CommandRecord, CommandValue } from "./view-model";

function isString(value: CommandValue): value is string {
  return typeof value === "string";
}

function isRecord(value: CommandValue): value is CommandRecord {
  return value !== null && typeof value === "object" && !Array.isArray(value);
}

function actionStringProp(props: CommandRecord, ...keys: string[]): string | null {
  for (const key of keys) {
    const value = props[key];
    if (isString(value) && value.trim().length > 0) return value;
  }
  return null;
}

function actionPathTargets(props: CommandRecord): string[] {
  const paths = props.paths;
  if (Array.isArray(paths)) {
    return paths.filter(
      (item): item is string => isString(item) && item.trim().length > 0,
    );
  }
  const single = actionStringProp(props, "path", "target");
  return single ? [single] : [];
}

export async function handleCommandAction(options: {
  action: CommandActionRequest;
  callback?: (payload?: CommandRecord) => void | Promise<void>;
  launcher?: (options: LaunchCommandOptions) => Promise<void>;
}): Promise<CommandActionResult> {
  const { action, callback, launcher } = options;
  // SAFETY: IPC action props are validated by the command snapshot serializer before dispatch.
  const props = action.props as CommandRecord;

  if (callback) {
    try {
      // SAFETY: callback payloads come from the command host's serialized action envelope.
      await callback(action.payload as CommandRecord | undefined);
      return { ok: true };
    } catch (err) {
      return { ok: false, error: err instanceof Error ? err.message : String(err) };
    }
  }

  if (action.type === "Action.CopyToClipboard") {
    const content = props.content;
    if (!isString(content)) return { ok: false, error: "missing_content" };
    clipboard.writeText(content);
    return { ok: true };
  }

  if (action.type === "Action.Paste") {
    const content = props.content;
    if (!isString(content)) return { ok: false, error: "missing_content" };
    clipboard.writeText(content);
    return { ok: true };
  }

  if (action.type === "Action.OpenInBrowser") {
    const url = props.url;
    if (!isString(url)) return { ok: false, error: "missing_url" };
    await shell.openExternal(url);
    return { ok: true };
  }

  if (action.type === "Action.Open") {
    const target = props.target;
    if (!isString(target)) return { ok: false, error: "missing_target" };
    if (/^https?:\/\//i.test(target)) {
      await shell.openExternal(target);
      return { ok: true };
    }

    const error = await shell.openPath(target);
    return error ? { ok: false, error } : { ok: true };
  }

  if (action.type === "Action.ShowInFinder") {
    // SAFETY: action props are produced by the normalized command snapshot serializer.
    const target = actionStringProp(props, "path", "target");
    if (!target) return { ok: false, error: "missing_path" };
    shell.showItemInFolder(target);
    return { ok: true };
  }

  if (action.type === "Action.Trash") {
    // SAFETY: action props are produced by the normalized command snapshot serializer.
    const targets = actionPathTargets(props);
    if (targets.length === 0) return { ok: false, error: "missing_path" };
    for (const target of targets) await shell.trashItem(target);
    return { ok: true };
  }

  if (action.type === "Action.LaunchCommand") {
    const name = props.name;
    if (!isString(name)) return { ok: false, error: "missing_name" };
    if (!launcher) return { ok: false, error: "launcher_not_configured" };
    const extensionName =
      isString(props.extensionName) ? props.extensionName : undefined;
    const fallbackText =
      isString(props.fallbackText) ? props.fallbackText : undefined;
    // SAFETY: Raycast launch options constrain the type field to LaunchCommandOptions values.
    const type = isString(props.type) ? (props.type as LaunchCommandOptions["type"]) : undefined;
    const args =
      props.arguments && isRecord(props.arguments)
        ? props.arguments
        : undefined;
    await launcher({
      name,
      extensionName,
      type,
      arguments: args,
      // SAFETY: Raycast launch context is an object when supplied by the command bridge.
      context: props.context as object | undefined,
      fallbackText,
    });
    return { ok: true };
  }

  return { ok: false, error: "unsupported_action" };
}
