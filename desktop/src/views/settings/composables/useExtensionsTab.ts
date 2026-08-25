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
// SAFETY: the surrounding domain validation preserves the asserted contract.
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
// SAFETY: the surrounding domain validation preserves the asserted contract.
      marketError.value = (e as Error).message;
    } finally {
      marketLoading.value = false;
    }
  }

  function catalogByAppId(appId: string | null): MarketplaceExtension | undefined {
    if (!appId) return undefined;
    return catalog.value?.extensions.find((e) => e.appId === appId);
  }

  function hasUpdate(i: InstalledExtensionInfo): boolean {
    const c = catalogByAppId(i.appId);
    return !!(c && i.version && c.version !== i.version);
  }

  async function onUpdate(i: InstalledExtensionInfo) {
    const c = catalogByAppId(i.appId);
    if (!c || installingId.value) return;
    installingId.value = i.id;
    marketError.value = "";
    try {
      await window.kepler.extension.installFromUrl(c.downloadUrl, c.sha256);
      await loadExtensions();
    } catch (e) {
// SAFETY: the surrounding domain validation preserves the asserted contract.
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
// SAFETY: the surrounding domain validation preserves the asserted contract.
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
// SAFETY: the surrounding domain validation preserves the asserted contract.
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
// SAFETY: the surrounding domain validation preserves the asserted contract.
      extensionsError.value = `${id}: ${(e as Error).message}`;
    } finally {
      busyExt.value = "";
    }
  }

  const availableInCatalog = computed<MarketplaceExtension[]>(() => {
    if (!catalog.value) return [];
    const installedAppIds = new Set(
      installed.value.map((i) => i.appId).filter((appId): appId is string => !!appId),
    );
    return catalog.value.extensions.filter(
      (c) => !c.appId || !installedAppIds.has(c.appId),
    );
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
    catalogByAppId,
    hasUpdate,
    onUpdate,
    onInstallNew,
    onRevert,
    onUninstall,
  };
}
