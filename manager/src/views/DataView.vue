<script setup lang="ts">
import { computed, onMounted, ref } from "vue";
import { SettingsList, SettingsRow } from "@kosmos/visuals";
import type { DataSummary } from "../manager-api";
import type { ManagerClient } from "../composables/useManagerClient";

const props = defineProps<{ client: ManagerClient }>();
type JsonValue = string | number | boolean | null | JsonRecord | JsonValue[];
interface JsonRecord {
  [key: string]: JsonValue;
}
type Row = JsonRecord;
const isRecord = (value: JsonValue): value is JsonRecord =>
  typeof value === "object" && value !== null && !Array.isArray(value);
const summary = ref<DataSummary | null>(null);
const types = ref<Row[]>([]);
const generation = ref(0);

function visibleType(value: Row): boolean {
  const id = String(value.id ?? value.name ?? "");
  return (
    value.internal !== true && value.hidden !== true && !id.startsWith("_")
  );
}

const summaryTypes = computed<Row[]>(() => {
  const value = summary.value?.types;
  return Array.isArray(value)
    ? value.filter(
        (item): item is Row =>
          typeof item === "object" && item !== null && visibleType(item),
      )
    : types.value.filter(visibleType);
});
const managedBytes = computed(() =>
  Number(summary.value?.managed_storage_bytes ?? 0),
);
const objectCount = computed(() =>
  Number(
    summary.value?.count ??
      summaryTypes.value.reduce(
        (total, item) => total + Number(item.count ?? 0),
        0,
      ),
  ),
);

async function refresh() {
  const request = generation.value + 1;
  generation.value = request;
  const [nextSummary, nextTypes] = await Promise.all([
    props.client.call<DataSummary>("getDataSummary", undefined, "summary"),
    props.client.call<unknown[]>("listObjectTypes", undefined, "types"),
  ]);
  if (request !== generation.value) return;
  summary.value = nextSummary;
  types.value = (nextTypes ?? []).filter(
    (item): item is Row =>
      typeof item === "object" && item !== null && visibleType(item),
  );
}

onMounted(() => void refresh());
</script>

<template>
  <section class="stack" aria-labelledby="data-heading">
    <SettingsList>
      <SettingsRow title="Типов данных" description="Доступные типы объектов">
        <template #control
          ><strong>{{ summaryTypes.length }}</strong></template
        >
      </SettingsRow>
      <SettingsRow title="Данных" description="Объекты, доступные менеджеру">
        <template #control
          ><strong>{{ objectCount }}</strong></template
        >
      </SettingsRow>
      <SettingsRow
        title="Управляемое хранилище"
        description="Данные движка и хранилище пакетов"
      >
        <template #control
          ><strong>{{ managedBytes }} Б</strong></template
        >
      </SettingsRow>
    </SettingsList>
    <SettingsList>
      <SettingsRow
        v-for="type in summaryTypes"
        :key="String(type.id ?? type.name)"
        :title="String(type.name ?? type.id)"
        :description="`${type.count ?? 0} объектов · ${type.logical_bytes ?? 0} Б логических данных`"
      />
      <SettingsRow
        v-if="!summaryTypes.length"
        title="Типы данных пока недоступны."
        muted
      />
    </SettingsList>
  </section>
</template>
