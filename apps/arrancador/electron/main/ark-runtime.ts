import {
  getArkDbPathForSelectedSpace,
  type SharedSelectedSpace,
} from "@kosmos/ark";

export interface ArkRuntimeBinding<TServices> {
  arkDbPath: string | null;
  services: TServices;
}

export function syncArkRuntimeBinding<TServices>(
  binding: ArkRuntimeBinding<TServices>,
  options: {
    appDataPath: string;
    selection: SharedSelectedSpace | null;
    createServices: (arkDbPath: string) => TServices;
  },
): ArkRuntimeBinding<TServices> & { changed: boolean } {
  const nextArkDbPath = getArkDbPathForSelectedSpace(
    options.appDataPath,
    options.selection,
  );

  if (binding.arkDbPath === nextArkDbPath) {
    return {
      ...binding,
      arkDbPath: nextArkDbPath,
      changed: false,
    };
  }

  return {
    arkDbPath: nextArkDbPath,
    services: options.createServices(nextArkDbPath),
    changed: true,
  };
}
