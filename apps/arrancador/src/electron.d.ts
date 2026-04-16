import type { ArrancadorBridge } from "@/types/ipc";

declare global {
  interface Window {
    arrancador?: ArrancadorBridge;
  }
}

export {};
