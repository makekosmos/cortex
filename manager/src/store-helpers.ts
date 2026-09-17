import type { InstalledStoreItem, StoreListing } from "./manager-api";

const canonicalPackageIds = new Set([
  "com.kosmos.shell",
  "com.kosmos.eden",
  "com.kosmos.agenda",
  "com.kosmos.arcadia",
  "com.kosmos.graph",
  "com.kosmos.dictation",
]);

// The Engine reports the host platform token on the store catalog snapshot
// (`store.catalog`/`store.refresh`). When the token is absent — a degraded or
// fixture context — listings stay unfiltered rather than blanking the store.
export function listingSupportsPlatform(listing: StoreListing, platform?: string) {
  return platform === undefined || (listing.availability?.platforms ?? []).includes(platform);
}

export function listingPackageId(listing: StoreListing): string | null {
  const distribution = listing.distribution;
  if (distribution && "package_id" in distribution) return distribution.package_id;
  return canonicalPackageIds.has(listing.id) ? listing.id : null;
}

export function latestInstalledPackages(installed: InstalledStoreItem[]) {
  const latest = new Map<string, InstalledStoreItem>();
  for (const item of installed) {
    const current = latest.get(item.id);
    if (
      !current ||
      current.version.localeCompare(item.version, undefined, {
        numeric: true,
      }) < 0
    )
      latest.set(item.id, item);
  }
  return [...latest.values()];
}

export function installedForListing(listing: StoreListing, installed: InstalledStoreItem[]) {
  const packageId = listingPackageId(listing);
  return packageId
    ? latestInstalledPackages(installed).find((item) => item.id === packageId)
    : undefined;
}

export function installTarget(listing: StoreListing, installed?: InstalledStoreItem) {
  const packageId = listingPackageId(listing);
  const distribution = listing.distribution;
  const catalogVersion =
    distribution && "package_id" in distribution ? distribution.version : undefined;
  const version = installed?.update_version ?? catalogVersion;
  if (!packageId || !version) return null;
  return {
    package_id: packageId,
    version,
  };
}

export function packageAction(listing: StoreListing, installed?: InstalledStoreItem) {
  if (installed?.revoked) return null;
  if (installed)
    return installed.update_version ? "update" : installed.kind === "app" ? "open" : null;
  return installTarget(listing) ? "install" : null;
}

export function installKey(listing: StoreListing, installed?: InstalledStoreItem): string {
  const target = installTarget(listing, installed);
  return target?.package_id ?? listing.id;
}
