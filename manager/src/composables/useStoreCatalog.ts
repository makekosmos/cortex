import { computed, ref } from "vue";
import type { ManagerClient } from "./useManagerClient";
import type {
  InstalledStoreItem,
  PackageSnapshot,
  StoreCatalogSnapshot,
  StoreListing,
} from "../manager-api";

export function useStoreCatalog(client: ManagerClient) {
  const snapshot = ref<StoreCatalogSnapshot | null>(null);
  const loading = ref(false);
  const error = ref<string | null>(null);
  const tab = ref("discover");
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
      const details = new Map(packages?.packages.map((item) => [item.id, item]) ?? []);
      snapshot.value = {
        ...result,
        installed: result.installed.map((item) => ({
          ...details.get(item.id),
          ...item,
        })),
      };
    } else error.value = "Каталог магазина недоступен.";
    loading.value = false;
  }
  const installed = computed<InstalledStoreItem[]>(() => snapshot.value?.installed ?? []);
  return {
    snapshot,
    loading,
    error,
    tab,
    listings,
    installed,
    load,
  };
}

export type { StoreListing };
