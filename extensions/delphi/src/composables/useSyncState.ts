import { shallowRef } from "vue";

export type ConnectionState = "online" | "syncing" | "offline";
export type ArkStatus = "connecting" | "connected" | "error";

export const activeSpaceCode = shallowRef<string | null>(null);
export const connectedPeerCount = shallowRef(0);
export const connectedPeerNames = shallowRef<string[]>([]);
export const connectionState = shallowRef<ConnectionState>("offline");
export const arkStatus = shallowRef<ArkStatus>("connecting");

export const LEAVE_SPACE_EVENT = "delphi:leave-space";

export function requestLeaveSpace(): void {
  window.dispatchEvent(new CustomEvent(LEAVE_SPACE_EVENT));
}
