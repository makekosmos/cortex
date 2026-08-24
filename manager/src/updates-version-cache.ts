export function createDesktopVersionCache(
  load: () => Promise<string | null | undefined>,
) {
  let cached: string | undefined;
  return async () => (cached ??= (await load()) ?? "—");
}
