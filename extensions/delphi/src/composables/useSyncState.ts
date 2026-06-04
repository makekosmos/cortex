import { shallowRef } from "vue";

export type ConnectionState = "online" | "syncing" | "offline";
export type ArkStatus = "connecting" | "connected" | "error";

export const connectionState = shallowRef<ConnectionState>("offline");
export const arkStatus = shallowRef<ArkStatus>("connecting");
