import type { IntegrationProvider, StoreListing } from "./manager-api";
import { listingPackageId, listingSupportsPlatform } from "./store-helpers";

export type ConnectionCard = {
  id: string;
  label: string;
  description?: string;
  iconUrl?: string;
  iconPath?: string;
  listing?: StoreListing;
  provider?: IntegrationProvider;
};

// TODO: return ARK Markdown Bridge to the marketplace after its user-facing flow is ready.
const HIDDEN_INTEGRATION_IDS = new Set(["ark-markdown-bridge"]);

const FIRST_PARTY_INTEGRATIONS: StoreListing[] = [
  {
    id: "com.kosmos.huawei-health",
    kind: "integration",
    name: "Huawei Health",
    description: "Импорт данных Huawei Health в ARK.",
    categories: ["integrations", "health"],
    availability: { platforms: ["windows"] },
  },
];

export function platformListings(listings: StoreListing[], platform?: string) {
  return listings.filter((listing) => listingSupportsPlatform(listing, platform));
}

function integrationId(listing: StoreListing) {
  return listingPackageId(listing) ?? listing.id;
}

function visibleIntegration(id: string) {
  return !HIDDEN_INTEGRATION_IDS.has(id);
}

export function integrationCards(
  listings: StoreListing[],
  providers: IntegrationProvider[],
  platform?: string,
): ConnectionCard[] {
  const byId = new Map(providers.map((provider) => [provider.id, provider]));
  const seen = new Set<string>();
  const cards: ConnectionCard[] = [];
  for (const listing of platformListings([...listings, ...FIRST_PARTY_INTEGRATIONS], platform)) {
    if (listing.kind !== "integration") continue;
    const id = integrationId(listing);
    if (!visibleIntegration(id) || seen.has(id)) continue;
    seen.add(id);
    const provider = byId.get(id);
    cards.push({
      id,
      label: provider?.label ?? listing.name,
      description: listing.description,
      iconUrl: listing.icon_url,
      iconPath: provider?.iconPath,
      listing,
      provider,
    });
  }
  for (const provider of providers) {
    if (!visibleIntegration(provider.id) || seen.has(provider.id)) continue;
    seen.add(provider.id);
    cards.push({
      id: provider.id,
      label: provider.label,
      iconPath: provider.iconPath,
      provider,
    });
  }
  return cards;
}
