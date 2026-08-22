export {};
declare global {
  interface Window {
    kosmosApp: {
      identity: unknown;
      window: { minimize(): void; close(): void };
      ark: {
        request(operation: string, params?: Record<string, unknown>): Promise<unknown>;
        subscribe(callback: (event: unknown) => void): () => void;
      };
      launcher: { request(operation: string, params?: Record<string, unknown>): Promise<unknown> };
    };
  }
}
