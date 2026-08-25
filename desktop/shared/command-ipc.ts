import type { IpcJsonObject } from "./ipc-json";

export interface CommandSnapshotNode {
  type: string;
  text?: string;
  props: IpcJsonObject;
  children: CommandSnapshotNode[];
}

export interface CommandSnapshot {
  sessionId: string;
  extensionId: string;
  extensionName: string;
  commandName: string;
  commandTitle: string;
  root: CommandSnapshotNode;
  createdAt: string;
}

export interface CommandFeedbackEvent {
  kind: "toast" | "hud";
  title: string;
  message?: string;
  style?: "success" | "failure" | "animated";
}

export interface CommandActionRequest {
  type: string;
  props: IpcJsonObject;
  payload?: IpcJsonObject;
}

export interface CommandActionResult {
  ok: boolean;
  error?: string;
}

export interface CommandFilePickerRequest {
  allowMultipleSelection?: boolean;
  canChooseDirectories?: boolean;
  canChooseFiles?: boolean;
  showHiddenFiles?: boolean;
}

export interface CommandFilePickerResult {
  ok: boolean;
  paths: string[];
  error?: string;
}
