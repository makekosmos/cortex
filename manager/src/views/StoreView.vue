<script setup lang="ts">
import { computed, onMounted, ref } from "vue";
import { Skeleton } from "@kosmos/visuals";
import type {
  DataSummary,
  DevelopmentPackage,
  InstalledStoreItem,
  StoreListing,
} from "../manager-api";
import type { ManagerClient } from "../composables/useManagerClient";
import { useStoreCatalog } from "../composables/useStoreCatalog";
import { installKey, installTarget, installedForListing } from "../store-helpers";
import { needsStoreDisclosure } from "../disclosure-helpers";
import { usePackageDisclosure } from "../composables/usePackageDisclosure";
import PermissionDisclosure from "./PermissionDisclosure.vue";
import StoreListingCard from "./StoreListingCard.vue";
import StoreDetail from "./StoreDetail.vue";
import StoreMarketplaceControls from "./StoreMarketplaceControls.vue";
import {
  filterListings,
  marketplaceTabs,
  permissionSummaries,
  recommendForData,
  type MarketplaceFilters,
  type MarketplaceTab,
} from "../store-catalog-helpers";

const props = defineProps<{ client: ManagerClient }>();
const emit = defineEmits<{ detailChange: [boolean] }>();
const { snapshot, loading, error, listings, installed, catalogPackages, load } = useStoreCatalog(
  props.client,
);
const disclosure = usePackageDisclosure(props.client);
const retiredListingIds = new Set([
  "com.kosmos.eden",
  "com.kosmos.delphi",
  "ark-markdown-bridge",
  "com.kosmos.ark-markdown-bridge",
  "external.obsidian",
]);
const installing = ref(new Set<string>());
const feedback = ref(new Map<string, { kind: "success" | "error"; message: string }>());
const selectedListing = ref<StoreListing | null>(null);
const development = ref<DevelopmentPackage[]>([]);
const dataSummary = ref<DataSummary | null>(null);
const activeTab = ref<MarketplaceTab>("discover");
const filters = ref<MarketplaceFilters>({
  platform: "all",
  kind: "all",
  category: "all",
  data: "all",
  fidelity: "all",
  install: "all",
});
const rowsBase = computed(() => {
  const apps = listings.value.filter(
    (item) => item.kind === "kosmos-package" && !retiredListingIds.has(item.id),
  );
  const local = development.value.map((item) => ({
    id: item.id,
    kind: "kosmos-package" as const,
    name: item.name,
    publisher: item.publisher,
    icon_url: item.icon_url,
    distribution: { package_id: item.id, version: item.version },
  }));
  return [...local, ...apps.filter((item) => !development.value.some((dev) => dev.id === item.id))];
});
const installedFor = (listing: StoreListing) => installedForListing(listing, installed.value);
const options = computed(() => ({
  platforms: [...new Set(rowsBase.value.flatMap((item) => item.availability?.platforms ?? []))],
  kinds: [...new Set(rowsBase.value.map((item) => item.kind))],
  categories: [...new Set(rowsBase.value.flatMap((item) => item.categories ?? []))],
  data: [
    ...new Set(
      rowsBase.value.flatMap((item) => (item.data_compatibility ?? []).map((entry) => entry.type)),
    ),
  ],
  fidelity: [
    ...new Set(
      rowsBase.value.flatMap((item) =>
        (item.data_compatibility ?? []).map((entry) => entry.fidelity),
      ),
    ),
  ],
}));
const rows = computed(() =>
  filterListings(rowsBase.value, installedFor, activeTab.value, filters.value),
);
const recommendations = computed(() => recommendForData(rowsBase.value, dataSummary.value));
const detail = computed(() => {
  const listing = selectedListing.value;
  if (!listing) return null;
  const catalog = catalogPackages.value.find(
    (item) => item.id === listing.distribution?.package_id,
  );
  return {
    listing,
    summary: listing.description ?? "Приложение для работы в экосистеме Kosmos.",
    metadata: [
      { label: "Автор", value: listing.publisher ?? "—" },
      { label: "Версия", value: catalog?.version ?? listing.distribution?.version ?? "—" },
      { label: "Размер", value: formatBytes(catalog?.archive_size) },
      { label: "Каталог обновлён", value: formatDate(snapshot.value?.issued_at) },
    ],
    permissions: permissionSummaries(installedFor(listing)),
    ...detailActionState(listing),
  };
});
function formatBytes(bytes?: number) {
  return bytes
    ? bytes >= 1024 * 1024
      ? `${(bytes / 1024 / 1024).toFixed(1)} МБ`
      : `${Math.ceil(bytes / 1024)} КБ`
    : "—";
}
function formatDate(value?: string) {
  return value
    ? new Intl.DateTimeFormat("ru-RU", { dateStyle: "medium" }).format(new Date(value))
    : "—";
}
function operationKey(listing: StoreListing) {
  return installKey(listing, installedFor(listing));
}
function detailActionState(listing: StoreListing) {
  const installed = installedFor(listing);
  if (listing.kind === "external-app") {
    const available = snapshot.value?.state === "fresh";
    return { actionLabel: available ? "Открыть сайт" : "Недоступно", actionDisabled: !available };
  }
  if (installed?.kind === "app") return { actionLabel: "Открыть", actionDisabled: false };
  if (installed?.update_version) return { actionLabel: "Обновить", actionDisabled: false };
  return {
    actionLabel: installed ? "Установлено" : "Установить",
    actionDisabled: Boolean(installed),
  };
}
function isInstalling(listing: StoreListing) {
  return installing.value.has(operationKey(listing));
}
function installFeedback(listing: StoreListing) {
  return feedback.value.get(operationKey(listing));
}
async function install(listing: StoreListing) {
  const target = installTarget(listing, installedFor(listing));
  if (!target || installing.value.has(target.package_id)) return;
  const developmentPackage = development.value.some((item) => item.id === listing.id);
  if (!needsStoreDisclosure(listing, developmentPackage)) return doInstall(target);
  await disclosure.request(target, listing.name, () => doInstall(target));
}
async function doInstall(target: { package_id: string; version: string }) {
  if (installing.value.has(target.package_id)) return;
  installing.value = new Set(installing.value).add(target.package_id);
  try {
    const result = await props.client.call(
      "installPackage",
      target,
      `store-install:${target.package_id}`,
    );
    feedback.value = new Map(feedback.value).set(target.package_id, {
      kind: result ? "success" : "error",
      message: result
        ? "Установлено"
        : (props.client.error.value ?? "Не удалось установить приложение."),
    });
    if (result) await load();
  } finally {
    const next = new Set(installing.value);
    next.delete(target.package_id);
    installing.value = next;
  }
}
async function openPackage(item: InstalledStoreItem) {
  if (
    !item.enabled &&
    !(await props.client.call(
      "setPackageEnabled",
      { package_id: item.id, version: item.version, enabled: true },
      `store-enable:${item.id}`,
    ))
  )
    return;
  await props.client.call("openPackage", { package_id: item.id }, `store-open:${item.id}`);
}
function showDetails(listing: StoreListing) {
  selectedListing.value = listing;
  emit("detailChange", true);
}
function openExternal(listing: StoreListing) {
  void props.client.call(
    "openStoreExternal",
    { listing_id: listing.id },
    `store-external:${listing.id}`,
  );
}
function runDetailAction() {
  if (!detail.value) return;
  const installed = installedFor(detail.value.listing);
  if (detail.value.listing.kind === "external-app") openExternal(detail.value.listing);
  else if (development.value.some((item) => item.id === detail.value!.listing.id))
    void props.client.call(
      "openDevelopmentPackage",
      { package_id: detail.value.listing.id },
      `store-dev-open:${detail.value.listing.id}`,
    );
  else if (installed?.kind === "app") void openPackage(installed);
  else if (!installed?.update_version) void install(detail.value.listing);
}
function backToCatalog() {
  selectedListing.value = null;
  emit("detailChange", false);
}
defineExpose({ backToCatalog });
onMounted(async () => {
  await load();
  void Promise.all([
    props.client.call("refreshPackageCatalog", undefined, "store-initial-package-catalog"),
    props.client.call("refreshStoreCatalog", undefined, "store-initial-catalog"),
  ]).then(() => load());
  development.value =
    (await props.client.call<DevelopmentPackage[]>("getDevelopmentPackages")) ?? [];
  dataSummary.value = await props.client.call<DataSummary>(
    "getDataSummary",
    undefined,
    "store-recommendations",
  );
});
</script>

<template>
  <section class="stack store-view" aria-label="Маркетплейс">
    <StoreDetail
      v-if="detail"
      v-bind="detail"
      :busy="isInstalling(detail.listing)"
      @action="runDetailAction"
    />
    <template v-else>
      <StoreMarketplaceControls
        :tabs="marketplaceTabs"
        :active-tab="activeTab"
        :filters="filters"
        :options="options"
        :recommendations="recommendations"
        :installed-for="installedFor"
        :catalog-available="snapshot?.state === 'fresh'"
        :installing="isInstalling"
        :feedback="installFeedback"
        @tab="activeTab = $event"
        @install="install"
        @open="openPackage"
        @external="openExternal"
        @details="showDetails"
      />
      <p v-if="error" class="error" role="alert">{{ error }}</p>
      <div v-if="loading && !rows.length" class="store-grid" aria-label="Загрузка приложений">
        <Skeleton v-for="index in 6" :key="index" class="h-24 w-full rounded-lg" />
      </div>
      <div v-else-if="rows.length" class="store-grid">
        <StoreListingCard
          v-for="listing in rows"
          :key="listing.id"
          :listing="listing"
          :installed="installedFor(listing)"
          :catalog-available="snapshot?.state === 'fresh'"
          :installing="isInstalling(listing)"
          :feedback="installFeedback(listing)"
          :development="development.some((entry) => entry.id === listing.id)"
          @install="install"
          @open="openPackage"
          @external="openExternal"
          @details="showDetails"
        />
      </div>
      <p v-else class="muted">Приложений не найдено.</p>
    </template>
    <PermissionDisclosure
      :open="disclosure.state.open"
      :name="disclosure.state.name"
      :sections="disclosure.state.sections"
      :unavailable="disclosure.state.unavailable"
      :busy="disclosure.state.busy"
      @confirm="disclosure.confirm"
      @decline="disclosure.decline"
    />
  </section>
</template>
