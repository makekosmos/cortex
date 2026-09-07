import type { DataSummary, InstalledStoreItem, StoreListing } from "./manager-api";

export const marketplaceTabs = [
  { id: "discover", label: "Открыть" },
  { id: "kosmos-apps", label: "Приложения Kosmos" },
  { id: "integrations", label: "Интеграции" },
  { id: "external-apps", label: "Внешние приложения" },
  { id: "installed", label: "Установленные" },
  { id: "updates", label: "Обновления" },
] as const;

export type MarketplaceTab = (typeof marketplaceTabs)[number]["id"];
export type MarketplaceFilters = {
  platform: string;
  kind: string;
  category: string;
  data: string;
  fidelity: string;
  install: "all" | "installed" | "available" | "update";
};

export function listingMatchesTab(
  listing: StoreListing,
  tab: MarketplaceTab,
  installed: InstalledStoreItem | undefined,
) {
  if (tab === "kosmos-apps") return listing.kind === "kosmos-package";
  if (tab === "integrations") return listing.kind === "integration";
  if (tab === "external-apps") return listing.kind === "external-app";
  if (tab === "installed") return Boolean(installed);
  if (tab === "updates") return Boolean(installed?.update_version);
  return true;
}

export function filterListings(
  listings: StoreListing[],
  installedFor: (listing: StoreListing) => InstalledStoreItem | undefined,
  tab: MarketplaceTab,
  filters: MarketplaceFilters,
) {
  return listings.filter((listing) => {
    const installed = installedFor(listing);
    const compatibility = listing.data_compatibility ?? [];
    if (!listingMatchesTab(listing, tab, installed)) return false;
    if (
      filters.platform !== "all" &&
      !(listing.availability?.platforms ?? []).includes(filters.platform)
    )
      return false;
    if (filters.kind !== "all" && listing.kind !== filters.kind) return false;
    if (filters.category !== "all" && !(listing.categories ?? []).includes(filters.category))
      return false;
    if (filters.data !== "all" && !compatibility.some((item) => item.type === filters.data))
      return false;
    if (
      filters.fidelity !== "all" &&
      !compatibility.some((item) => item.fidelity === filters.fidelity)
    )
      return false;
    if (filters.install === "installed" && !installed) return false;
    if (filters.install === "available" && installed) return false;
    if (filters.install === "update" && !installed?.update_version) return false;
    return true;
  });
}

export function recommendForData(listings: StoreListing[], summary: DataSummary | null) {
  const types = new Set((summary?.types ?? []).map((item) => item.type_id));
  return listings
    .map((listing) => ({
      listing,
      matches: (listing.data_compatibility ?? []).filter((item) => types.has(item.type)),
    }))
    .filter((item) => item.matches.length)
    .sort((a, b) => b.matches.length - a.matches.length)
    .map((item) => item.listing);
}

export function permissionSummaries(item: InstalledStoreItem | undefined) {
  return (item?.effective_grants ?? []).map((grant) => {
    const reads = grant.fields_read.length
      ? `читает: ${grant.fields_read.join(", ")}`
      : "без чтения полей";
    const writes = grant.fields_write.length
      ? `изменяет: ${grant.fields_write.join(", ")}`
      : "без записи полей";
    return `${grant.type} · ${grant.roles.join(", ") || "данные"} · ${reads}; ${writes}`;
  });
}
