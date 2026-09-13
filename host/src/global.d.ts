export {};
type HostJsonValue =
  | string
  | number
  | boolean
  | null
  | readonly HostJsonValue[]
  | { readonly [key: string]: HostJsonValue };
type HostApiParams = Readonly<{ [key: string]: HostJsonValue }>;
type HostAppOpenRequest = Readonly<{ id: string; route?: string }>;
type HostAppOpenResult = { ok: true } | { ok: false; message: string };
declare global {
  interface Window {
    kosmosApp: {
      identity: HostJsonValue;
      window: { minimize(): void; close(): void };
      dialogs: {
        pickDirectoryGrant(): Promise<{ persistentGrantId: string; label: string } | null>;
      };
      ark: {
        request(operation: string, params?: HostApiParams): Promise<HostJsonValue>;
        subscribe(callback: (event: HostJsonValue) => void): () => void;
      };
      launcher: { request(operation: string, params?: HostApiParams): Promise<HostJsonValue> };
      apps: { open(request: HostAppOpenRequest): Promise<HostAppOpenResult> };
      navigation: { onNavigate(handler: (route: string) => void): () => void };
    };
  }
}
