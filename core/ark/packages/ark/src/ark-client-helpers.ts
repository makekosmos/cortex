import type { SidecarRequest } from "./ark-client.types.js";

const LOCAL_WRITE_OPERATIONS = new Set([
  "upsert_todo",
  "delete_todo",
  "batch_upsert_todos",
  "upsert_project",
  "delete_project",
  "upsert_area",
  "upsert_tag",
  "upsert_heading",
  "delete_heading",
  "upsert_tracked_app",
  "delete_tracked_app",
  "upsert_usage_session",
  "delete_usage_session",
  "upsert_usage_event",
  "delete_usage_event",
  "upsert_object",
  "delete_object",
  "upsert_object_type",
  "delete_object_type",
  "upsert_object_link",
  "delete_object_link",
]);

export function withLocalWriteDeviceId(req: SidecarRequest, deviceId: string): SidecarRequest {
  if (!LOCAL_WRITE_OPERATIONS.has(req.operation)) return req;
  return {
    ...req,
    device_id: deviceId,
  };
}

export function generateHlc(deviceId: string): string {
  const now = new Date().toISOString();
  const counter = String(Math.floor(Math.random() * 999999)).padStart(6, "0");
  return `${now}:${counter}:${deviceId}`;
}
