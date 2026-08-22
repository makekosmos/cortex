<script setup lang="ts">
import { computed, onMounted, ref } from "vue";
import { Button, SettingsButtonRow, SettingsList, SettingsRow, TextInput } from "@kosmos/visuals";
import type {
  BridgeConfig,
  BridgeWorkerStatus,
  PackageItem,
  PackageSnapshot,
  PackageTrustStatus,
} from "../manager-api";
import type { ManagerClient } from "../composables/useManagerClient";

const props = defineProps<{ client: ManagerClient }>();
const filter = ref<"all" | "app" | "source" | "bridge">("all");
const snapshot = ref<PackageSnapshot | null>(null);
const trust = ref<PackageTrustStatus | null>(null);
type BridgeDraft = BridgeConfig & {
  health: string;
  status?: BridgeWorkerStatus;
};
const bridge = ref<Record<string, BridgeDraft>>({});
const bridgeKey = (item: PackageItem) => `${item.id}@${item.version}`;
const items = computed<PackageItem[]>(() => {
  return [...(snapshot.value?.packages ?? []), ...(snapshot.value?.catalog ?? [])];
});
function kindLabel(kind?: string): string {
  if (kind === "app") return "приложение";
  if (kind === "source") return "источник";
  if (kind === "bridge") return "мост";
  return "пакет";
}
function statusLabel(status?: string): string {
  if (status === "running" || status === "healthy" || status === "ready") return "работает";
  if (status === "stopped" || status === "disabled") return "остановлен";
  if (status === "restarting") return "перезапускается";
  if (status === "failed" || status === "error") return "ошибка";
  return status ? "состояние изменилось" : "состояние не сообщено";
}
function packageDescription(item: PackageItem): string {
  const identity = `${kindLabel(item.kind)} · ${item.version} · ${item.publisher}`;
  if (item.revoked)
    return `${identity} · отозван: ${item.revocation_reason ?? "пакет заблокирован"}`;
  return `${identity} · рабочий процесс: ${statusLabel(
    bridge.value[bridgeKey(item)]?.health ?? item.worker_health ?? item.worker_state,
  )}`;
}
function selectFilter(kind: "all" | "app" | "source" | "bridge") {
  filter.value = kind;
  void refresh();
}

async function refresh() {
  const [nextSnapshot, nextTrust] = await Promise.all([
    props.client.call<PackageSnapshot>(
      "getPackages",
      filter.value === "all" ? undefined : { kind: filter.value },
      "packages",
    ),
    props.client.call<PackageTrustStatus>("getPackageTrustStatus", undefined, "package-trust"),
  ]);
  snapshot.value = nextSnapshot;
  trust.value = nextTrust;
  await Promise.all(
    items.value
      .filter((item) => item.kind === "bridge" && item.version)
      .map((item) => loadBridge(item)),
  );
}
async function loadBridge(item: PackageItem) {
  const result = await props.client.call<{
    config?: BridgeConfig;
    worker_state: string;
    worker_status?: BridgeWorkerStatus;
  }>(
    "getBridgeConfig",
    { package_id: item.id, version: item.version },
    `bridge:${bridgeKey(item)}`,
  );
  const config = result?.config ?? {
    vault_root: "",
    selected_types: [],
    editable_fields: ["title", "body"],
    readonly_fields: [],
  };
  bridge.value[bridgeKey(item)] = {
    ...config,
    health: result?.worker_state ?? "stopped",
    status: result?.worker_status,
  };
}
function fields(value: string): string[] {
  return value
    .split(",")
    .map((item) => item.trim())
    .filter(Boolean);
}
async function saveBridge(item: PackageItem) {
  const draft = bridge.value[bridgeKey(item)];
  if (!draft) return;
  const result = await props.client.call<{ worker_state: string }>(
    "setBridgeConfig",
    {
      package_id: item.id,
      version: item.version,
      config: {
        vault_root: draft.vault_root.trim(),
        selected_types: draft.selected_types,
        editable_fields: draft.editable_fields,
        readonly_fields: draft.readonly_fields,
      },
    },
    `bridge:${bridgeKey(item)}`,
  );
  if (result) {
    draft.health = result.worker_state;
    draft.status = result.worker_status;
  }
}
async function refreshCatalog() {
  await props.client.call("refreshPackageCatalog", undefined, "catalog");
  await refresh();
}
async function install(item: PackageItem) {
  if (item.revoked || trust.value?.state !== "usable") return;
  await props.client.call(
    "installPackage",
    { package_id: item.id, version: item.update_version ?? item.version },
    `install:${item.id}`,
  );
  await refresh();
}
async function toggle(item: PackageItem) {
  if (item.revoked || !window.confirm(item.enabled ? "Отключить пакет?" : "Включить пакет?"))
    return;
  await props.client.call(
    "setPackageEnabled",
    { package_id: item.id, version: item.version, enabled: !item.enabled },
    `enable:${item.id}`,
  );
  await refresh();
}
async function uninstall(item: PackageItem) {
  if (!window.confirm("Удалить этот пакет?")) return;
  await props.client.call(
    "uninstallPackage",
    { package_id: item.id, version: item.version },
    `uninstall:${item.id}`,
  );
  await refresh();
}
onMounted(() => void refresh());
</script>

<template>
  <section class="stack settings-page">
    <SettingsList>
      <SettingsRow title="Фильтр" description="Какие пакеты показывать">
        <template #control>
          <div class="segmented" aria-label="Фильтр пакетов">
            <Button
              v-for="kind in ['all', 'app', 'source', 'bridge']"
              :key="kind"
              size="sm"
              :variant="filter === kind ? 'primary' : 'ghost'"
              :aria-pressed="filter === kind"
              @click="selectFilter(kind as 'all' | 'app' | 'source' | 'bridge')"
              >{{
                kind === "all"
                  ? "Все"
                  : kind === "app"
                    ? "Приложения"
                    : kind === "source"
                      ? "Источники"
                      : "Мосты"
              }}</Button
            >
          </div>
        </template>
      </SettingsRow>
      <SettingsButtonRow
        title="Каталог"
        description="Установку выполняет движок из подписанного каталога."
        button-label="Обновить каталог"
        @click="refreshCatalog"
      />
      <SettingsRow
        title="Доверие каталогу"
        :description="trust?.message ?? 'Состояние доверия каталога неизвестно.'"
        :muted="trust?.state === 'usable'"
        data-testid="package-trust"
      />
    </SettingsList>
    <SettingsList>
      <template v-for="item in items" :key="`${item.id}@${item.version}`">
        <SettingsRow :title="item.name ?? item.id" :description="packageDescription(item)">
          <template #control>
            <Button
              v-if="item.catalog && !item.revoked"
              size="sm"
              :disabled="trust?.state !== 'usable'"
              @click="install(item)"
              >Установить</Button
            ><Button
              v-if="!item.catalog && !item.revoked"
              variant="ghost"
              size="sm"
              :disabled="item.revoked"
              @click="toggle(item)"
              >{{ item.enabled ? "Отключить" : "Включить" }}</Button
            ><Button
              v-if="!item.catalog && !item.revoked && item.update_version"
              variant="ghost"
              size="sm"
              @click="install(item)"
              >Обновить до {{ item.update_version }}</Button
            ><Button v-if="!item.catalog" variant="danger" size="sm" @click="uninstall(item)"
              >Удалить</Button
            >
          </template>
        </SettingsRow>
        <p
          v-if="item.kind === 'bridge' && bridge[bridgeKey(item)]?.status"
          class="bridge-status muted"
        >
          Последняя синхронизация:
          {{ bridge[bridgeKey(item)].status?.last_sync ?? "нет" }} · конфликтов:
          {{ bridge[bridgeKey(item)].status?.conflict_count ?? 0
          }}<span v-if="bridge[bridgeKey(item)].status?.last_conflict_at">
            · последний конфликт:
            {{ bridge[bridgeKey(item)].status?.last_conflict_at }}</span
          >
        </p>
        <details
          v-if="item.kind === 'bridge' && item.version && bridge[bridgeKey(item)]"
          class="bridge-config"
        >
          <summary>Настроить мост</summary>
          <p class="muted">
            Укажите существующую папку vault. Движок остановит мост, заменит доступ к папке и
            запустит его заново.
          </p>
          <label
            >Папка vault<TextInput
              v-model="bridge[bridgeKey(item)].vault_root"
              autocomplete="off"
              placeholder="C:\\Заметки\\Vault" /></label
          ><label
            >Типы ARK<TextInput
              :model-value="bridge[bridgeKey(item)].selected_types.join(', ')"
              placeholder="note"
              @update:model-value="
                bridge[bridgeKey(item)].selected_types = fields($event)
              " /></label
          ><label
            >Редактируемые поля<TextInput
              :model-value="bridge[bridgeKey(item)].editable_fields.join(', ')"
              placeholder="title, body"
              @update:model-value="
                bridge[bridgeKey(item)].editable_fields = fields($event)
              " /></label
          ><label
            >Только чтение<TextInput
              :model-value="bridge[bridgeKey(item)].readonly_fields.join(', ')"
              @update:model-value="
                bridge[bridgeKey(item)].readonly_fields = fields($event)
              " /></label
          ><Button size="sm" @click="saveBridge(item)">Сохранить настройки</Button>
        </details>
      </template>
      <SettingsRow v-if="!items.length" title="Пакеты не найдены" />
    </SettingsList>
  </section>
</template>
