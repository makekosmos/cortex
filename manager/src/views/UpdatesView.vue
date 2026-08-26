<script setup lang="ts">
import { computed, onBeforeUnmount, onMounted, ref } from "vue";
import { SettingsList } from "@kosmos/visuals";
import type {
  DesktopUpdateState,
  InstalledStoreItem,
  StoreListing,
} from "../manager-api";
import type { ManagerClient } from "../composables/useManagerClient";
import { installTarget, listingPackageId, packageAction } from "../store-helpers";
import UpdatesRow from "./UpdatesRow.vue";
import desktopIcon from "../../../desktop/build/icon.png";
import { createDesktopVersionCache } from "../updates-version-cache";

const props = defineProps<{ client: ManagerClient }>();
const desktopVersion = ref("—");
const desktop = ref<DesktopUpdateState>({ kind: "idle" });
const listings = ref<StoreListing[]>([]);
const installed = ref<InstalledStoreItem[]>([]);
const packagesAvailable = ref(false);
const busy = ref<string | null>(null);
let timer: ReturnType<typeof setInterval> | undefined;

const loadDesktopVersion = createDesktopVersionCache(() =>
  props.client.call<string>("getAppVersion", undefined, "updates-version"),
);
const rows = computed(() =>
  installed.value.map((installedItem) => {
    const listing = listings.value.find(
      (item) => listingPackageId(item) === installedItem.id,
    );
    return {
      id: installedItem.id,
      name: installedItem.name ?? listing?.name ?? installedItem.id,
      icon: installedItem.icon_path
        ? `file:///${installedItem.icon_path.replace(/\\/g, "/")}`
        : desktopIcon,
      listing,
      installedItem,
    };
  }),
);

function desktopStatus() {
  if (desktop.value.kind === "checking") return "Проверяем…";
  if (desktop.value.kind === "downloading")
    return `Скачивание ${desktop.value.percent}%`;
  if (desktop.value.kind === "downloaded") return "Готово к установке";
  if (desktop.value.kind === "available") return "Скачивание…";
  if (desktop.value.kind === "error") return `Ошибка: ${desktop.value.message}`;
  return "";
}
function appStatus(item: (typeof rows.value)[number]) {
  if (!packagesAvailable.value)
    return "Пакеты недоступны: проверьте соединение и повторите проверку.";
  if (item.installedItem?.revoked) return "Пакет отозван.";
  if (!item.installedItem && !item.listing)
    return "Опубликованной версии пока нет.";
  return "";
}
async function load() {
  desktopVersion.value = await loadDesktopVersion();
  const [state, catalog, packages] = await Promise.all([
    props.client.call<DesktopUpdateState>(
      "getDesktopUpdateState",
      undefined,
      "desktop-update-state",
    ),
    props.client.call<{ listings: StoreListing[] }>(
      "getStoreCatalog",
      undefined,
      "updates-catalog",
    ),
    props.client.call<{ packages: InstalledStoreItem[] }>(
      "getPackages",
      { kind: "app" },
      "updates-packages",
    ),
  ]);
  if (state) desktop.value = state;
  if (catalog) listings.value = catalog.listings;
  if (packages) installed.value = packages.packages;
  packagesAvailable.value = catalog !== null && packages !== null;
}
function actionFor(item: (typeof rows.value)[number]) {
  return packageAction(
    item.listing ?? { id: item.id, kind: "kosmos-package", name: item.name },
    item.installedItem,
  );
}
function actionLabel(item: (typeof rows.value)[number]) {
  const action = actionFor(item);
  return action === "install"
    ? "Установить"
    : action === "update"
      ? "Обновить"
      : action === "open"
        ? "Открыть"
        : undefined;
}
async function update(
  item: (typeof rows.value)[number],
): Promise<string | null> {
  const listing = item.listing ?? {
    id: item.id,
    kind: "kosmos-package" as const,
    name: item.name,
  };
  const action = actionFor(item);
  const target = installTarget(listing, item.installedItem);
  if (!action || busy.value) return null;
  busy.value = item.id;
  try {
    if (action === "open") {
      if (
        await props.client.call(
          "openPackage",
          { package_id: item.id },
          `updates-open:${item.id}`,
        )
      )
        return null;
      return `${item.name}: ${props.client.error.value ?? "ошибка запуска"}`;
    }
    if (
      !target ||
      !(await props.client.call(
        "installPackage",
        target,
        `updates-install:${item.id}`,
      ))
    )
      return `${item.name}: ${props.client.error.value ?? "ошибка обновления"}`;
    await load();
    return null;
  } finally {
    busy.value = null;
  }
}
async function installDesktop() {
  await props.client.call(
    "installDesktopUpdate",
    undefined,
    "desktop-update-install",
  );
  await load();
}
onMounted(() => {
  void Promise.all([
    props.client.call("refreshPackageCatalog", undefined, "updates-initial-package-catalog"),
    props.client.call("refreshStoreCatalog", undefined, "updates-initial-store-catalog"),
    props.client.call<DesktopUpdateState>("checkDesktopUpdates", undefined, "desktop-update-check"),
  ]).finally(() => void load());
  timer = setInterval(() => void load(), 1500);
});
onBeforeUnmount(() => clearInterval(timer));
</script>

<template>
  <section class="stack updates-view" aria-label="Обновления">
    <SettingsList>
      <UpdatesRow
        title="Kosmos Desktop"
        id="desktop"
        :icon="desktopIcon"
        :current="desktopVersion"
        :available="
          desktop.kind === 'available' ||
          desktop.kind === 'downloading' ||
          desktop.kind === 'downloaded'
            ? desktop.version
            : null
        "
        :status="desktopStatus()"
        :action-label="
          desktop.kind === 'downloaded'
            ? 'Перезапустить и установить'
            : undefined
        "
        :disabled="desktop.kind !== 'downloaded'"
        @action="installDesktop"
      />
    </SettingsList>
    <hr />
    <h2 class="updates-heading">Приложения Kosmos</h2>
    <SettingsList>
      <UpdatesRow
        v-for="item in rows"
        :key="item.id"
        :title="item.name"
        :id="item.id"
        :icon="item.icon"
        :current="
          packagesAvailable
            ? (item.installedItem?.version ??
              item.listing?.distribution?.version ??
              '—')
            : '—'
        "
        :available="
          packagesAvailable ? item.installedItem?.update_version : null
        "
        :status="appStatus(item)"
        :action-label="actionLabel(item)"
        :disabled="
          !packagesAvailable ||
          item.installedItem?.revoked === true ||
          !actionFor(item)
        "
        :busy="busy === item.id"
        @action="update(item)"
      />
    </SettingsList>
  </section>
</template>
