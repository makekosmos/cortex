<script setup lang="ts">
import { computed, onMounted, ref } from "vue";
import { Button, SettingsList, SettingsRow, TextInput } from "@kosmos/visuals";
import type {
  DevEnvironment,
  DevelopmentPackage,
  PackageItem,
  PackageSnapshot,
} from "../manager-api";
import type { ManagerClient } from "../composables/useManagerClient";
import { usePackageDisclosure } from "../composables/usePackageDisclosure";
import PermissionDisclosure from "./PermissionDisclosure.vue";

const props = defineProps<{ client: ManagerClient }>();
const disclosure = usePackageDisclosure(props.client);
const environment = ref<DevEnvironment | null>(null);
const development = ref<DevelopmentPackage[]>([]);
const snapshot = ref<PackageSnapshot | null>(null);
const installPath = ref("");
const pathFeedback = ref<{ kind: "success" | "error"; message: string } | null>(null);
const busy = ref(new Set<string>());

const installed = computed(() => snapshot.value?.packages ?? []);
const devRows = computed(() =>
  development.value.map((entry) => ({
    entry,
    installed: installed.value.find(
      (item) => item.id === entry.id && item.version === entry.version,
    ),
  })),
);
function kindLabel(kind?: string): string {
  if (kind === "app") return "приложение";
  if (kind === "source") return "источник";
  if (kind === "bridge") return "мост";
  return "пакет";
}
function describeInstalled(item: PackageItem): string {
  const parts = [kindLabel(item.kind), item.version, item.publisher];
  if (item.catalog_sequence === 0) parts.push("dev");
  if (item.revoked) parts.push("отозван");
  if (item.worker_state) parts.push(`воркер: ${item.worker_state}`);
  return parts.join(" · ");
}
async function refresh() {
  const [env, dev, next] = await Promise.all([
    props.client.call<DevEnvironment>("getDevEnvironment", undefined, "dev-env"),
    props.client.call<DevelopmentPackage[]>("getDevelopmentPackages", undefined, "dev-packages"),
    props.client.call<PackageSnapshot>("getPackages", undefined, "packages"),
  ]);
  environment.value = env;
  development.value = dev ?? [];
  snapshot.value = next;
}
async function guarded(key: string, action: () => Promise<void>) {
  if (busy.value.has(key)) return;
  busy.value.add(key);
  try {
    await action();
  } finally {
    busy.value.delete(key);
  }
}
async function installDevelopment(entry: DevelopmentPackage) {
  await guarded(`dev:${entry.id}`, async () => {
    const result = await props.client.call(
      "installDevelopmentPackage",
      { package_id: entry.id },
      `dev-install:${entry.id}`,
    );
    if (result) await refresh();
  });
}
async function installFromPath() {
  const value = installPath.value.trim();
  if (!value) return;
  await guarded("path-install", async () => {
    pathFeedback.value = null;
    const result = await props.client.call<{ package_id: string; version: string }>(
      "installDevelopmentPath",
      { path: value },
      "dev-install-path",
    );
    if (result) {
      pathFeedback.value = {
        kind: "success",
        message: `Установлен ${result.package_id}@${result.version}`,
      };
      installPath.value = "";
      await refresh();
    } else {
      pathFeedback.value = {
        kind: "error",
        message: props.client.error.value ?? "Не удалось установить пакет.",
      };
    }
  });
}
async function openDevelopment(entry: DevelopmentPackage) {
  await guarded(`dev-open:${entry.id}`, async () => {
    await props.client.call(
      "openDevelopmentPackage",
      { package_id: entry.id },
      `dev-open:${entry.id}`,
    );
  });
}
async function openInstalled(item: PackageItem) {
  await props.client.call("openPackage", { package_id: item.id }, `open:${item.id}`);
}
async function toggle(item: PackageItem) {
  await guarded(`toggle:${item.id}`, async () => {
    await props.client.call(
      "setPackageEnabled",
      { package_id: item.id, version: item.version, enabled: !item.enabled },
      `enable:${item.id}`,
    );
    await refresh();
  });
}
async function uninstall(item: PackageItem) {
  if (!window.confirm(`Удалить ${item.id}@${item.version} из этого инстанса?`)) return;
  await guarded(`uninstall:${item.id}`, async () => {
    await props.client.call(
      "uninstallPackage",
      { package_id: item.id, version: item.version },
      `uninstall:${item.id}`,
    );
    await refresh();
  });
}
function preview(item: { id: string; name?: string; version?: string }) {
  void disclosure.request(
    { package_id: item.id, version: item.version },
    item.name ?? item.id,
    () => {},
  );
}
onMounted(() => void refresh());
</script>

<template>
  <section class="stack settings-page">
    <SettingsList>
      <SettingsRow
        title="Dev-инстанс"
        :description="
          environment?.run_id
            ? `run ${environment.run_id}`
            : 'Запущен вне dev-run — данные не изолированы.'
        "
      />
      <SettingsRow
        v-if="environment?.data_dir"
        title="Данные движка"
        :description="environment.data_dir"
      />
    </SettingsList>
    <SettingsList>
      <SettingsRow
        title="Локальные пакеты"
        description="Из dev-packages.json. Для app-пакетов dev-сервер поднимает `pnpm run dev`."
      />
      <SettingsRow
        v-for="row in devRows"
        :key="row.entry.id"
        :title="`${row.entry.name} ${row.entry.version}`"
        :description="`${row.entry.id} · ${row.entry.source_path ?? ''}${row.installed ? ' · установлен' : ''}`"
      >
        <template #control>
          <Button
            variant="surface"
            size="sm"
            :disabled="busy.has(`dev:${row.entry.id}`)"
            @click="installDevelopment(row.entry)"
            >{{ row.installed ? "Переустановить" : "Установить" }}</Button
          ><Button v-if="row.installed" variant="surface" size="sm" @click="preview(row.entry)"
            >Разрешения</Button
          ><Button
            v-if="row.entry.source_path"
            variant="surface"
            size="sm"
            :disabled="busy.has(`dev-open:${row.entry.id}`)"
            @click="openDevelopment(row.entry)"
            >Открыть</Button
          >
        </template>
      </SettingsRow>
      <SettingsRow v-if="!devRows.length" title="Локальные пакеты не настроены" />
    </SettingsList>
    <SettingsList>
      <SettingsRow
        title="Установить по пути"
        description="Путь к .kspkg или к директории пакета (manifest + release/*.kspkg)."
      />
      <SettingsRow title="" description="">
        <template #control>
          <div class="dev-path-row">
            <TextInput
              v-model="installPath"
              autocomplete="off"
              placeholder="C:\\repos\\my-package или …\\release\\pkg-1.0.0.kspkg"
            />
            <Button
              variant="surface"
              size="sm"
              :disabled="busy.has('path-install') || !installPath.trim()"
              @click="installFromPath"
              >Установить</Button
            >
          </div>
        </template>
      </SettingsRow>
      <p v-if="pathFeedback" :class="pathFeedback.kind === 'error' ? 'error' : 'muted'">
        {{ pathFeedback.message }}
      </p>
    </SettingsList>
    <SettingsList>
      <SettingsRow title="Установленные пакеты" description="Состояние этого инстанса движка." />
      <SettingsRow
        v-for="item in installed"
        :key="`${item.id}@${item.version}`"
        :title="item.name ?? item.id"
        :description="describeInstalled(item)"
      >
        <template #control>
          <Button
            v-if="item.kind === 'app'"
            variant="surface"
            size="sm"
            @click="openInstalled(item)"
            >Открыть</Button
          ><Button variant="surface" size="sm" @click="preview(item)">Разрешения</Button
          ><Button
            v-if="!item.revoked"
            variant="surface"
            size="sm"
            :disabled="busy.has(`toggle:${item.id}`)"
            @click="toggle(item)"
            >{{ item.enabled ? "Отключить" : "Включить" }}</Button
          ><Button
            variant="danger"
            size="sm"
            :disabled="busy.has(`uninstall:${item.id}`)"
            @click="uninstall(item)"
            >Удалить</Button
          >
        </template>
      </SettingsRow>
      <SettingsRow v-if="!installed.length" title="Пакеты не установлены" />
    </SettingsList>
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

<style scoped>
.dev-path-row {
  display: flex;
  gap: 8px;
  align-items: center;
  min-width: 420px;
}
.dev-path-row :deep(input) {
  flex: 1;
}
</style>
