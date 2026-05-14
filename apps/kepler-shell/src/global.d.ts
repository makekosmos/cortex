import type { KeplerApi } from "@shared/ipc-types";

declare global {
  interface Window {
    kepler: KeplerApi;
  }
}

export {};
