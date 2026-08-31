export type JsonValue =
  | string
  | number
  | boolean
  | null
  | readonly JsonValue[]
  | { readonly [key: string]: JsonValue };
export type Manifest = { id: string; version: string; icon?: string; [key: string]: JsonValue };
export type Permission = { capability: string; scopes?: readonly string[] };
export type PackageArchive = { file: string; manifest: Manifest };
