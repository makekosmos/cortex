import type {
  UserDataDeleteResult,
  UserDataReadResult,
  UserDataStatResult,
  UserDataWriteResult,
} from "../electron/extension-user-data-ipc";
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
      userData: {
        read(key: string): Promise<UserDataReadResult>;
        write(key: string, bytes: Uint8Array): Promise<UserDataWriteResult>;
        delete(key: string): Promise<UserDataDeleteResult>;
        stat(key: string): Promise<UserDataStatResult>;
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
