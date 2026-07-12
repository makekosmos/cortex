/// <reference types="vite/client" />

declare module "@kosmos/visuals/theme/css";

interface Window {
  kepler?: {
    ark: {
      request<T = unknown>(operation: string, params?: Record<string, unknown>): Promise<T>;
      subscribe(event: string, handler: (payload: unknown) => void): () => void;
    };
    dialogs: { pickDirectory(): Promise<string | null> };
  };
}
