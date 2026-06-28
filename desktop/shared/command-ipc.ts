export interface CommandSnapshotNode {
  type: string;
  text?: string;
  props: Record<string, unknown>;
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
  props: Record<string, unknown>;
  payload?: Record<string, unknown>;
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
