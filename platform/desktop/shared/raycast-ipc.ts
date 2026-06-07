export interface RaycastSnapshotNode {
  type: string;
  text?: string;
  props: Record<string, unknown>;
  children: RaycastSnapshotNode[];
}

export interface RaycastSnapshot {
  sessionId: string;
  extensionId: string;
  extensionName: string;
  commandName: string;
  commandTitle: string;
  root: RaycastSnapshotNode;
  createdAt: string;
}

export interface RaycastFeedbackEvent {
  kind: "toast" | "hud";
  title: string;
  message?: string;
  style?: "success" | "failure" | "animated";
}

export interface RaycastActionRequest {
  type: string;
  props: Record<string, unknown>;
  payload?: Record<string, unknown>;
}

export interface RaycastActionResult {
  ok: boolean;
  error?: string;
}

export interface RaycastFilePickerRequest {
  allowMultipleSelection?: boolean;
  canChooseDirectories?: boolean;
  canChooseFiles?: boolean;
  showHiddenFiles?: boolean;
}

export interface RaycastFilePickerResult {
  ok: boolean;
  paths: string[];
  error?: string;
}
