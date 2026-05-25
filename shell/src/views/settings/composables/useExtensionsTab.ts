// useExtensionsTab — installed + marketplace catalog + install/update/revert/uninstall.

import { computed, ref } from "vue";
import type {
  InstalledExtensionInfo,
  MarketplaceCatalog,
  MarketplaceExtension,
} from "@shared/ipc-types";

export function useExtensionsTab() {
  const installed = ref<InstalledExtensionInfo[]>([]);
  const extensionsLoading = ref<boolean>(false);
  const extensionsError = ref<string>("");
  const busyExt = ref<string>("");

  const catalog = ref<MarketplaceCatalog | null>(null);
  const marketLoading = ref<boolean>(false);
  const marketError = ref<string>("");
  const installingId = ref<string>("");

  async function loadExtensions() {
    extensionsLoading.value = true;
    extensionsError.value = "";
    try {
      installed.value = await window.kepler.extension.installedList();
    } catch (e) {
      extensionsError.value = (e as Error).message;
    } finally {
      extensionsLoading.value = false;
    }
  }

  async function loadCatalog(force = false) {
    marketLoading.value = true;
    marketError.value = "";
    try {
      catalog.value = await window.kepler.extension.catalogFetch(force);
    } catch (e) {
      marketError.value = (e as Error).message;
    } finally {
      marketLoading.value = false;
    }
  }

  function catalogById(id: string): MarketplaceExtension | undefined {
    return catalog.value?.extensions.find((e) => e.id === id);
  }

  function hasUpdate(i: InstalledExtensionInfo): boolean {
    const c = catalogById(i.id);
    return !!(c && i.version && c.version !== i.version);
  }

  async function onUpdate(i: InstalledExtensionInfo) {
    const c = catalogById(i.id);
    if (!c || installingId.value) return;
    installingId.value = i.id;
    marketError.value = "";
    try {
      await window.kepler.extension.installFromUrl(c.downloadUrl, c.sha256);
      await loadExtensions();
    } catch (e) {
      marketError.value = `${i.id}: ${(e as Error).message}`;
    } finally {
      installingId.value = "";
    }
  }

  async function onInstallNew(c: MarketplaceExtension) {
    if (installingId.value) return;
    installingId.value = c.id;
    marketError.value = "";
    try {
      await window.kepler.extension.installFromUrl(c.downloadUrl, c.sha256);
      await loadExtensions();
    } catch (e) {
      marketError.value = `${c.id}: ${(e as Error).message}`;
    } finally {
      installingId.value = "";
    }
  }

  async function onRevert(id: string) {
    if (busyExt.value) return;
    busyExt.value = id;
    extensionsError.value = "";
    try {
      const ok = await window.kepler.extension.revert(id);
      if (!ok) {
        extensionsError.value = `${id}: нет доступных backup'ов для отката`;
      }
      await loadExtensions();
    } catch (e) {
      extensionsError.value = `${id}: ${(e as Error).message}`;
    } finally {
      busyExt.value = "";
    }
  }

  async function onUninstall(id: string) {
    if (busyExt.value) return;
    busyExt.value = id;
    extensionsError.value = "";
    try {
      await window.kepler.extension.uninstall(id);
      await loadExtensions();
    } catch (e) {
      extensionsError.value = `${id}: ${(e as Error).message}`;
    } finally {
      busyExt.value = "";
    }
  }

  const availableInCatalog = computed<MarketplaceExtension[]>(() => {
    if (!catalog.value) return [];
    const installedIds = new Set(installed.value.map((i) => i.id));
    return catalog.value.extensions.filter((c) => !installedIds.has(c.id));
  });

  return {
    installed,
    extensionsLoading,
    extensionsError,
    busyExt,
    catalog,
    marketLoading,
    marketError,
    installingId,
    availableInCatalog,
    loadExtensions,
    loadCatalog,
    catalogById,
    hasUpdate,
    onUpdate,
    onInstallNew,
    onRevert,
    onUninstall,
  };
}
