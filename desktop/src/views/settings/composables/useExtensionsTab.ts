// useExtensionsTab — Runtime .kspkg applications shown in Settings.

import { computed, ref } from "vue";
import { isRecord, isString } from "../../../shared/runtimeGuards";
import type { IpcJsonObject, IpcJsonValue } from "@shared/ipc-json";

type RuntimePackage = {
  id: string;
  name: string;
  version: string;
  kind: string;
  publisher: string;
  enabled?: boolean;
  revoked?: boolean;
  update_version?: string | null;
};

function unwrap(value: IpcJsonValue): IpcJsonValue {
  if (isRecord(value) && value.ok === false) {
    throw new Error(isString(value.error) ? value.error : "Операция Runtime не выполнена.");
  }
  return isRecord(value) && "data" in value ? value.data : value;
}

function isRuntimePackage(value: IpcJsonValue): value is IpcJsonObject & RuntimePackage {
  return (
    isRecord(value) &&
    isString(value.id) &&
    isString(value.name) &&
    isString(value.version) &&
    isString(value.kind) &&
    isString(value.publisher)
  );
}

function runtimePackages(value: IpcJsonValue | undefined): RuntimePackage[] {
  if (!Array.isArray(value)) return [];
  const packages: RuntimePackage[] = [];
  for (const item of value) if (isRuntimePackage(item)) packages.push(item);
  return packages;
}

function message(cause: unknown): string {
  return cause instanceof Error ? cause.message : "Не удалось выполнить операцию с приложением.";
}

export function useExtensionsTab() {
  const installed = ref<RuntimePackage[]>([]);
  const catalog = ref<RuntimePackage[]>([]);
  const extensionsLoading = ref(false);
  const extensionsError = ref("");
  const busyExt = ref("");

  async function request(operation: string, params: IpcJsonObject = {}): Promise<IpcJsonValue> {
    return unwrap(await window.kepler.ark.request<IpcJsonValue>(operation, params));
  }

  async function loadCatalog(refresh = false) {
    extensionsLoading.value = true;
    extensionsError.value = "";
    try {
      if (refresh) await request("packages.refresh_catalog");
      const next = await request("packages.list", { kind: "app" });
      if (!isRecord(next)) throw new Error("Runtime вернул некорректный каталог приложений.");
      installed.value = runtimePackages(next.packages);
      catalog.value = runtimePackages(next.catalog);
    } catch (cause) {
      extensionsError.value = message(cause);
    } finally {
      extensionsLoading.value = false;
    }
  }

  const catalogById = (id: string) => catalog.value.find((item) => item.id === id);
  const hasUpdate = (item: RuntimePackage) => !!item.update_version;

  async function install(item: RuntimePackage) {
    if (busyExt.value) return;
    busyExt.value = item.id;
    extensionsError.value = "";
    try {
      await request("packages.install", { id: item.id, version: item.version });
      await loadCatalog();
    } catch (cause) {
      extensionsError.value = `${item.name}: ${message(cause)}`;
    } finally {
      busyExt.value = "";
    }
  }

  async function onToggle(item: RuntimePackage) {
    if (busyExt.value) return;
    busyExt.value = item.id;
    extensionsError.value = "";
    try {
      await request("packages.set_enabled", {
        id: item.id,
        version: item.version,
        enabled: !item.enabled,
      });
      await loadCatalog();
    } catch (cause) {
      extensionsError.value = `${item.name}: ${message(cause)}`;
    } finally {
      busyExt.value = "";
    }
  }

  const availableInCatalog = computed(() => {
    const installedIds = new Set(installed.value.map((item) => item.id));
    return catalog.value.filter((item) => !installedIds.has(item.id) && !item.revoked);
  });

  return {
    installed,
    extensionsLoading,
    extensionsError,
    busyExt,
    availableInCatalog,
    loadCatalog,
    catalogById,
    hasUpdate,
    install,
    onToggle,
  };
}
