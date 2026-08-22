import type { ManagerApi } from "./manager-api";

declare global {
  interface Window {
    kosmosManager: ManagerApi;
  }
}

export {};
