import { computed, ref, watch } from "vue";
import type { ManagerClient } from "./useManagerClient";
import type {
  InstalledStoreItem,
  PackageItem,
  PackageSnapshot,
  StoreCatalogSnapshot,
  StoreListing,
} from "../manager-api";

export function useStoreCatalog(client: ManagerClient) {
  const snapshot = ref<StoreCatalogSnapshot | null>(null);
  const loading = ref(false);
  const error = ref<string | null>(null);
  const tab = ref("discover");
  const catalogPackages = ref<PackageItem[]>([]);
  const listings = computed(() =>
    (snapshot.value?.listings ?? []).filter((listing) =>
      (listing.availability?.platforms ?? []).includes("windows"),
    ),
  );
  async function load(refresh = false) {
    loading.value = true;
    error.value = null;
    const result = await client.call<StoreCatalogSnapshot>(
      refresh ? "refreshStoreCatalog" : "getStoreCatalog",
      undefined,
      "store",
    );
    if (result) {
      const packages = await client.call<PackageSnapshot>(
        "getPackages",
        undefined,
        "store-packages",
      );
      catalogPackages.value = packages?.catalog ?? [];
      const details = new Map(packages?.packages.map((item) => [item.id, item]) ?? []);
      snapshot.value = {
        ...result,
        installed: result.installed.map((item) => ({
          ...details.get(item.id),
          ...item,
        })),
      };
    } else error.value = client.banner.value ? "Каталог магазина недоступен." : null;
    loading.value = false;
  }
  // Engine recovery: refill the catalog once connectivity is back instead of
  // leaving an empty store behind a healed transient gap.
  watch(client.engine, (state) => {
    if (state === "ready" && snapshot.value === null && !loading.value) void load();
  });
  const installed = computed<InstalledStoreItem[]>(() => snapshot.value?.installed ?? []);
  return {
    snapshot,
    loading,
    error,
    tab,
    listings,
    installed,
    catalogPackages,
    load,
  };
}

export type { StoreListing };
