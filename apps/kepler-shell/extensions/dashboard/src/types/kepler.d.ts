// Minimal typing для window.kepler — выставляется shared extension-preload.

export {};

declare global {
  interface Window {
    kepler?: {
      ark: {
        request: <T = unknown>(
          operation: string,
          params?: Record<string, unknown>,
        ) => Promise<T>;
        subscribe: (
          event: string,
          handler: (payload: unknown) => void,
        ) => () => void;
      };
      meta: {
        id: () => Promise<string | null>;
      };
      window: {
        close: () => Promise<void>;
        minimize: () => Promise<void>;
        maximize: () => Promise<void>;
      };
      host: {
        invoke: (action: string, payload?: unknown) => Promise<boolean>;
      };
    };
  }
}
