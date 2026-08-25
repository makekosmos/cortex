export {};
type HostJsonValue =
  | string
  | number
  | boolean
  | null
  | readonly HostJsonValue[]
  | { readonly [key: string]: HostJsonValue };
type HostApiParams = Readonly<{ [key: string]: HostJsonValue }>;
declare global {
  interface Window {
    kosmosApp: {
      identity: HostJsonValue;
      window: { minimize(): void; close(): void };
      ark: {
        request(operation: string, params?: HostApiParams): Promise<HostJsonValue>;
        subscribe(callback: (event: HostJsonValue) => void): () => void;
      };
      launcher: { request(operation: string, params?: HostApiParams): Promise<HostJsonValue> };
    };
  }
}
