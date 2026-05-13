import type { HorologionApi } from "@shared/ipc-types";

declare global {
  interface Window {
    horologion: HorologionApi;
  }
}

export {};
