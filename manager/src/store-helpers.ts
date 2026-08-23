import type { InstalledStoreItem, StoreListing } from "./manager-api";

const canonicalPackageIds = new Set([
  "com.kosmos.shell",
  "com.kosmos.eden",
  "com.kosmos.agenda",
  "com.kosmos.graph",
  "com.kosmos.dictation",
]);

export function listingPackageId(listing: StoreListing): string | null {
  const distribution = listing.distribution;
  if (distribution && "package_id" in distribution)
    return distribution.package_id;
  return canonicalPackageIds.has(listing.id) ? listing.id : null;
}

export function installedForListing(
  listing: StoreListing,
  installed: InstalledStoreItem[],
) {
  const packageId = listingPackageId(listing);
  return packageId
    ? installed.find((item) => item.id === packageId)
    : undefined;
}

export function installTarget(
  listing: StoreListing,
  installed?: InstalledStoreItem,
) {
  const packageId = listingPackageId(listing);
  const distribution = listing.distribution;
  const catalogVersion =
    distribution && "package_id" in distribution
      ? distribution.version
      : undefined;
  const version = installed?.update_version ?? catalogVersion;
  if (!packageId || !version) return null;
  return {
    package_id: packageId,
    version,
  };
}

export function packageAction(
  listing: StoreListing,
  installed?: InstalledStoreItem,
) {
  if (installed?.revoked) return null;
  if (installed)
    return installed.update_version
      ? "update"
      : installed.kind === "app"
        ? "open"
        : null;
  return installTarget(listing) ? "install" : null;
}

export function installKey(
  listing: StoreListing,
  installed?: InstalledStoreItem,
): string {
  const target = installTarget(listing, installed);
  return target?.package_id ?? listing.id;
}
