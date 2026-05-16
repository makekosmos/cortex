import type { HorologionExtensionApi } from "./lib/horologionApi";

declare global {
  interface Window {
    horologion: HorologionExtensionApi;
    kepler?: {
      ark: {
        request: <T = unknown>(operation: string, params?: Record<string, unknown>) => Promise<T>;
        subscribe: (event: string, handler: (payload: unknown) => void) => () => void;
      };
      window?: {
        close(): Promise<void>;
        minimize(): Promise<void>;
        maximize(): Promise<void>;
      };
      navigation?: {
        onNavigate(handler: (route: string) => void): () => void;
        initialRoute(): Promise<string | null>;
      };
    };
  }
}

export {};
