<script setup lang="ts">
import { computed, onMounted, ref } from "vue";
import { Button, SettingsList, SettingsRow } from "@kosmos/visuals";
import type { InstalledStoreItem, StoreListing } from "../manager-api";
import type { ManagerClient } from "../composables/useManagerClient";
import { useStoreCatalog } from "../composables/useStoreCatalog";
import {
  installKey,
  installTarget,
  installedForListing,
} from "../store-helpers";
import StoreListingCard from "./StoreListingCard.vue";
import shellIcon from "../../../desktop/build/icon.png";
import memoriaIcon from "../../../desktop/build/app-icons/memoria.png";
import agendaIcon from "../../../desktop/build/app-icons/agenda.png";
import arcadiaIcon from "../../../desktop/build/app-icons/arcadia.png";

const props = defineProps<{ client: ManagerClient }>();
const emit = defineEmits<{ detailChange: [boolean] }>();
const { snapshot, error, listings, installed, catalogPackages, load } = useStoreCatalog(
  props.client,
);
const installing = ref(new Set<string>());
const feedback = ref(
  new Map<string, { kind: "success" | "error"; message: string }>(),
);
const selectedListing = ref<StoreListing | null>(null);

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
    name: "Memoria",
    publisher: "Kosmos",
    icon_url: memoriaIcon,
  },
  {
    id: "com.kosmos.delphi",
    kind: "kosmos-package",
    name: "Agenda",
    publisher: "Kosmos",
    icon_url: agendaIcon,
  },
  {
    id: "com.kosmos.arcadia",
    kind: "kosmos-package",
    name: "Arcadia",
    publisher: "Kosmos",
    icon_url: arcadiaIcon,
  },
];
const rows = computed(() => {
  const apps = listings.value.filter(
    (listing) => listing.kind === "kosmos-package",
  );
  const canonical = canonicalApps.map((app) => {
    const listing = apps.find((candidate) => candidate.id === app.id);
    return listing
      ? { ...listing, ...app }
      : app;
  });
  return [
    ...canonical,
    ...apps.filter(
      (listing) => !canonicalApps.some((app) => app.id === listing.id),
    ),
  ];
});
const detail = computed(() => {
  const listing = selectedListing.value;
  if (!listing) return null;
  const summary = listing.description ??
    ({
      "com.kosmos.shell": "Рабочее пространство Kosmos для команд, данных и приложений.",
      "com.kosmos.eden": "Личное пространство для заметок и знаний.",
      "com.kosmos.delphi": "Планирование задач, встреч и повседневных дел.",
      "com.kosmos.arcadia": "Пространство для игр и игровых данных в Kosmos.",
    }[listing.id] ?? "Приложение для работы в экосистеме Kosmos.");
  const catalogPackage = catalogPackages.value.find(
    (item) => item.id === listing.distribution?.package_id,
  );
  return {
    listing,
    summary,
    metadata: [
      { label: "Автор", value: listing.publisher ?? "—" },
      { label: "Версия", value: catalogPackage?.version ?? listing.distribution?.version ?? "—" },
      { label: "Размер", value: formatBytes(catalogPackage?.archive_size) },
      { label: "Каталог обновлён", value: formatDate(snapshot.value?.issued_at) },
    ],
    permissions: [
      "Данные приложения — для сохранения вашей работы.",
      "Локальное хранилище — данные остаются на этом устройстве.",
    ],
  };
});
function formatBytes(bytes?: number) {
  if (!bytes) return "—";
  return bytes >= 1024 * 1024
    ? `${(bytes / 1024 / 1024).toFixed(1)} МБ`
    : `${Math.ceil(bytes / 1024)} КБ`;
}
function formatDate(value?: string) {
  return value
    ? new Intl.DateTimeFormat("ru-RU", { dateStyle: "medium" }).format(new Date(value))
    : "—";
}
const detailAction = computed(() => {
  const listing = selectedListing.value;
  if (!listing) return null;
  const item = installedFor(listing);
  if (item?.kind === "app") return { label: "Открыть", item };
  const target = installTarget(listing, item);
  return target && snapshot.value?.state === "fresh"
    ? { label: "Установить", item: undefined }
    : { label: "Недоступно", item: undefined, disabled: true };
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
  await props.client.call(
    "openPackage",
    { package_id: item.id },
    `store-open:${item.id}`,
  );
}
function runDetailAction() {
  if (!detailAction.value) return;
  if (detailAction.value.item) void openPackage(detailAction.value.item);
  else if (!detailAction.value.disabled && selectedListing.value)
    void install(selectedListing.value);
}
function showDetails(listing: StoreListing) {
  selectedListing.value = listing;
  emit("detailChange", true);
}
function backToCatalog() {
  selectedListing.value = null;
  emit("detailChange", false);
}
defineExpose({ backToCatalog });
onMounted(() => void load());
</script>

<template>
  <section class="stack store-view" aria-label="Маркетплейс">
    <template v-if="detail">
      <section class="store-detail-app-header">
        <img v-if="detail.listing.icon_url" :src="detail.listing.icon_url" alt="" />
        <div class="store-detail-app-copy">
          <h1>{{ detail.listing.name }}</h1>
          <p>{{ detail.summary }}</p>
        </div>
        <Button
          v-if="detailAction"
          size="sm"
          variant="surface"
          :disabled="detailAction.disabled || isInstalling(detail.listing)"
          @click="runDetailAction"
        >{{ isInstalling(detail.listing) ? "Установка…" : detailAction.label }}</Button>
      </section>
      <div class="store-detail-gallery" aria-label="Превью приложения">
        <div
          v-for="index in 2"
          :key="index"
          class="store-detail-shot"
        >
          <img v-if="detail.listing.icon_url" :src="detail.listing.icon_url" alt="" />
        </div>
      </div>
      <dl class="store-detail-metadata">
        <div v-for="item in detail.metadata" :key="item.label">
          <dt>{{ item.label }}</dt>
          <dd>{{ item.value }}</dd>
        </div>
      </dl>
      <SettingsList>
        <SettingsRow title="О приложении" :description="detail.summary" stacked />
      </SettingsList>
      <SettingsList>
        <SettingsRow title="Разрешения" description="Приложению потребуется:" />
        <SettingsRow
          v-for="permission in detail.permissions"
          :key="permission"
          title=""
          :description="permission"
        />
      </SettingsList>
    </template>
    <template v-else>
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
        @details="showDetails"
      />
      </div>
      <p v-else class="muted">Приложений не найдено.</p>
    </template>
  </section>
</template>
