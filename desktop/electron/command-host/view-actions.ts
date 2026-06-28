import { clipboard, shell } from "electron";
import type { LaunchCommandOptions } from "@raycast/api";
import type { CommandActionRequest, CommandActionResult } from "../../shared/command-ipc";

function actionStringProp(props: Record<string, unknown>, ...keys: string[]): string | null {
  for (const key of keys) {
    const value = props[key];
    if (typeof value === "string" && value.trim().length > 0) return value;
  }
  return null;
}

function actionPathTargets(props: Record<string, unknown>): string[] {
  const paths = props.paths;
  if (Array.isArray(paths)) {
    return paths.filter(
      (item): item is string => typeof item === "string" && item.trim().length > 0,
    );
  }
  const single = actionStringProp(props, "path", "target");
  return single ? [single] : [];
}

export async function handleCommandAction(options: {
  action: CommandActionRequest;
  callback?: (payload?: Record<string, unknown>) => unknown | Promise<unknown>;
  launcher?: (options: LaunchCommandOptions) => Promise<void>;
}): Promise<CommandActionResult> {
  const { action, callback, launcher } = options;

  if (callback) {
    try {
      await callback(action.payload);
      return { ok: true };
    } catch (err) {
      return { ok: false, error: err instanceof Error ? err.message : String(err) };
    }
  }

  if (action.type === "Action.CopyToClipboard") {
    const content = action.props.content;
    if (typeof content !== "string") return { ok: false, error: "missing_content" };
    clipboard.writeText(content);
    return { ok: true };
  }

  if (action.type === "Action.Paste") {
    const content = action.props.content;
    if (typeof content !== "string") return { ok: false, error: "missing_content" };
    clipboard.writeText(content);
    return { ok: true };
  }

  if (action.type === "Action.OpenInBrowser") {
    const url = action.props.url;
    if (typeof url !== "string") return { ok: false, error: "missing_url" };
    await shell.openExternal(url);
    return { ok: true };
  }

  if (action.type === "Action.Open") {
    const target = action.props.target;
    if (typeof target !== "string") return { ok: false, error: "missing_target" };
    if (/^https?:\/\//i.test(target)) {
      await shell.openExternal(target);
      return { ok: true };
    }

    const error = await shell.openPath(target);
    return error ? { ok: false, error } : { ok: true };
  }

  if (action.type === "Action.ShowInFinder") {
    const target = actionStringProp(action.props, "path", "target");
    if (!target) return { ok: false, error: "missing_path" };
    shell.showItemInFolder(target);
    return { ok: true };
  }

  if (action.type === "Action.Trash") {
    const targets = actionPathTargets(action.props);
    if (targets.length === 0) return { ok: false, error: "missing_path" };
    for (const target of targets) await shell.trashItem(target);
    return { ok: true };
  }

  if (action.type === "Action.LaunchCommand") {
    const name = action.props.name;
    if (typeof name !== "string") return { ok: false, error: "missing_name" };
    if (!launcher) return { ok: false, error: "launcher_not_configured" };
    const extensionName =
      typeof action.props.extensionName === "string" ? action.props.extensionName : undefined;
    const fallbackText =
      typeof action.props.fallbackText === "string" ? action.props.fallbackText : undefined;
    const type =
      typeof action.props.type === "string"
        ? (action.props.type as LaunchCommandOptions["type"])
        : undefined;
    const args =
      action.props.arguments && typeof action.props.arguments === "object"
        ? (action.props.arguments as Record<string, unknown>)
        : undefined;
    await launcher({
      name,
      extensionName,
      type,
      arguments: args,
      context: action.props.context,
      fallbackText,
    });
    return { ok: true };
  }

  return { ok: false, error: "unsupported_action" };
}
