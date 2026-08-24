<script setup lang="ts">
import { computed, onMounted, ref } from "vue";
import { Button, SettingsList, SettingsRow, TextInput } from "@kosmos/visuals";
import type { DataSummary } from "../manager-api";
import type { ManagerClient } from "../composables/useManagerClient";

const props = defineProps<{ client: ManagerClient }>();
type Row = Record<string, unknown>;
const summary = ref<DataSummary | null>(null);
const types = ref<Row[]>([]);
const rows = ref<Row[]>([]);
const query = ref("");
const cursor = ref<string | undefined>();
const selectedType = ref("");
const generation = ref(0);

function visibleType(value: Row): boolean {
  const id = String(value.id ?? value.name ?? "");
  return value.internal !== true && value.hidden !== true && !id.startsWith("_");
}

const summaryTypes = computed<Row[]>(() => {
  const value = summary.value?.types;
  return Array.isArray(value)
    ? value.filter(
        (item): item is Row => typeof item === "object" && item !== null && visibleType(item),
      )
    : types.value.filter(visibleType);
});
const managedBytes = computed(() => Number(summary.value?.managed_storage_bytes ?? 0));
const objectCount = computed(() =>
  Number(
    summary.value?.count ??
      summaryTypes.value.reduce((total, item) => total + Number(item.count ?? 0), 0),
  ),
);

function metadata(value: Row): Row | null {
  if (value.deleted_at) return null;
  return {
    id: value.id,
    type: value.type_id ?? value.type ?? value.object_type,
    title: value.title ?? value.name ?? value.id,
    created_at: value.created_at,
    updated_at: value.updated_at,
  };
}

async function loadObjects(request: number, append = false) {
  const result = await props.client.call<unknown>(
    "listObjects",
    {
      type_id: selectedType.value || undefined,
      limit: 100,
      cursor: cursor.value,
    },
    "objects",
  );
  if (request !== generation.value || !result) return;
  const payload = result as Row;
  const values = Array.isArray(result)
    ? result
    : Array.isArray(payload.rows)
      ? payload.rows
      : payload.items;
  const page = Array.isArray(values)
    ? values
        .filter((item): item is Row => typeof item === "object" && item !== null)
        .map(metadata)
        .filter((item): item is Row => item !== null)
    : [];
  rows.value = append ? [...rows.value, ...page] : page;
  cursor.value = typeof payload.next_cursor === "string" ? payload.next_cursor : undefined;
}

async function refresh() {
  const request = generation.value + 1;
  generation.value = request;
  cursor.value = undefined;
  const [nextSummary, nextTypes] = await Promise.all([
    props.client.call<DataSummary>("getDataSummary", undefined, "summary"),
    props.client.call<unknown[]>("listObjectTypes", undefined, "types"),
  ]);
  if (request !== generation.value) return;
  summary.value = nextSummary;
  types.value = (nextTypes ?? []).filter(
    (item): item is Row => typeof item === "object" && item !== null && visibleType(item),
  );
  await loadObjects(request);
}

async function search() {
  const value = query.value.trim().slice(0, 256);
  if (!value) return;
  const request = generation.value + 1;
  generation.value = request;
  cursor.value = undefined;
  const result = await props.client.call<unknown>("searchObjects", { query: value }, "search");
  if (request !== generation.value || !result) return;
  const payload = result as Row;
  const values = Array.isArray(result)
    ? result
    : Array.isArray(payload.rows)
      ? payload.rows
      : payload.items;
  rows.value = Array.isArray(values)
    ? values
        .filter((item): item is Row => typeof item === "object" && item !== null)
        .map(metadata)
        .filter((item): item is Row => item !== null)
    : [];
}

function selectType(type: string) {
  selectedType.value = type;
  void refresh();
}

onMounted(() => void refresh());
</script>

<template>
  <section class="stack" aria-labelledby="data-heading">
    <div class="toolbar">
      <TextInput
        v-model="query"
        class="toolbar-input"
        aria-label="Поиск объектов"
        maxlength="256"
        placeholder="Поиск по данным"
        @keyup.enter="search"
      />
      <Button size="sm" :disabled="!query.trim()" @click="search">Найти</Button>
      <Button variant="ghost" size="sm" @click="refresh">Обновить</Button>
    </div>
    <div class="segmented" aria-label="Фильтр типов">
      <Button
        size="sm"
        :variant="!selectedType ? 'primary' : 'ghost'"
        :aria-pressed="!selectedType"
        @click="selectType('')"
        >Все типы</Button
      >
      <Button
        v-for="type in types"
        :key="String(type.id ?? type.name)"
        size="sm"
        :variant="selectedType === String(type.id ?? type.name) ? 'primary' : 'ghost'"
        :aria-pressed="selectedType === String(type.id ?? type.name)"
        @click="selectType(String(type.id ?? type.name))"
        >{{ type.name ?? type.id }}</Button
      >
    </div>
    <SettingsList>
      <SettingsRow title="Типов данных" description="Доступные типы объектов">
        <template #control
          ><strong>{{ summaryTypes.length }}</strong></template
        >
      </SettingsRow>
      <SettingsRow title="Живых объектов" description="Объекты, доступные менеджеру">
        <template #control
          ><strong>{{ objectCount }}</strong></template
        >
      </SettingsRow>
      <SettingsRow title="Управляемое хранилище" description="Данные движка и хранилище пакетов">
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
      <SettingsRow v-if="!summaryTypes.length" title="Типы данных пока недоступны." muted />
    </SettingsList>
    <SettingsList aria-labelledby="data-heading">
      <h2 id="data-heading" class="list-title settings-list__title">Метаданные объектов</h2>
      <SettingsRow
        v-for="row in rows"
        :key="String(row.id)"
        :title="String(row.title ?? row.id)"
        :description="`${row.type ?? 'Тип не указан'} · ${row.updated_at ?? 'Дата не указана'}`"
      />
      <SettingsRow v-if="!rows.length" title="Нет объектов для отображения." muted />
      <Button v-if="cursor" variant="ghost" size="sm" @click="loadObjects(generation, true)"
        >Показать ещё</Button
      >
    </SettingsList>
  </section>
</template>
