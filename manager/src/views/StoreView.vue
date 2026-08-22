<script setup lang="ts">
import { computed, onMounted, ref } from "vue";
import type { InstalledStoreItem, StoreListing } from "../manager-api";
import type { ManagerClient } from "../composables/useManagerClient";
import { useStoreCatalog } from "../composables/useStoreCatalog";
import { installKey, installTarget, installedForListing } from "../store-helpers";
import StoreListingCard from "./StoreListingCard.vue";
import shellIcon from "../../../../products/shell/icon.ico";
import edenIcon from "../../../../products/eden/icon.png";
import delphiIcon from "../../../../products/delphi/icon.png";
import dictationIcon from "../../../../products/dictation/icon.ico";
import graphIcon from "../../../../products/cosmos-graph/icon.ico";

const props = defineProps<{ client: ManagerClient }>();
const { snapshot, error, listings, installed, load } = useStoreCatalog(props.client);
const installing = ref(new Set<string>());
const feedback = ref(new Map<string, { kind: "success" | "error"; message: string }>());

const canonicalApps: StoreListing[] = [
  {
    id: "com.kosmos.shell",
    kind: "kosmos-package",
    name: "Kosmos Shell",
    publisher: "Kosmos",
    icon_url: shellIcon,
  },
  {
    id: "com.kosmos.eden",
    kind: "kosmos-package",
    name: "Eden",
    publisher: "Kosmos",
    icon_url: edenIcon,
  },
  {
    id: "com.kosmos.delphi",
    kind: "kosmos-package",
    name: "Delphi",
    publisher: "Kosmos",
    icon_url: delphiIcon,
  },
  {
    id: "com.kosmos.graph",
    kind: "kosmos-package",
    name: "Cosmos Graph",
    publisher: "Kosmos",
    icon_url: graphIcon,
  },
  {
    id: "com.kosmos.dictation",
    kind: "kosmos-package",
    name: "Dictation",
    publisher: "Kosmos",
    icon_url: dictationIcon,
  },
];
const rows = computed(() => {
  const apps = listings.value.filter((listing) => listing.kind === "kosmos-package");
  const canonical = canonicalApps.map((app) => {
    const listing = apps.find((candidate) => candidate.id === app.id);
    return listing ? { ...app, ...listing, icon_url: listing.icon_url ?? app.icon_url } : app;
  });
  return [
    ...canonical,
    ...apps.filter((listing) => !canonicalApps.some((app) => app.id === listing.id)),
  ];
});
function installedFor(listing: StoreListing) {
  return installedForListing(listing, installed.value);
}
function operationKey(listing: StoreListing) {
  return installKey(listing, installedFor(listing));
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
  if (!item.enabled) {
    const enabled = await props.client.call(
      "setPackageEnabled",
      { package_id: item.id, version: item.version, enabled: true },
      `store-enable:${item.id}`,
    );
    if (!enabled) return;
    await load();
  }
  await props.client.call("openPackage", { package_id: item.id }, `store-open:${item.id}`);
}
onMounted(() => void load());
</script>

<template>
  <section class="stack store-view" aria-label="Маркетплейс">
    <p v-if="error" class="error" role="alert">{{ error }}</p>
    <div v-if="rows.length" class="store-grid">
      <StoreListingCard
        v-for="listing in rows"
        :key="listing.id"
        :listing="listing"
        :installed="installedFor(listing)"
        :catalog-available="snapshot?.state === 'fresh'"
        :installing="isInstalling(listing)"
        :feedback="installFeedback(listing)"
        @install="install"
        @open="openPackage"
      />
    </div>
    <p v-else class="muted">Приложений не найдено.</p>
  </section>
</template>
