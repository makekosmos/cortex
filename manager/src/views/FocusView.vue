<script setup lang="ts">
import { computed, onMounted, ref } from "vue";
import {
  Button,
  SettingsButtonRow,
  SettingsList,
  SettingsRow,
  StatusDot,
  TextInput,
  Textarea,
} from "@kosmos/visuals";
import type { FocusBlocklist, FocusServiceStatus } from "../manager-api";
import type { ManagerClient } from "../composables/useManagerClient";
import {
  parseFocusDomains,
  type FocusUpsertParams,
} from "./focus-view-helpers";
const props = defineProps<{ client: ManagerClient }>();
const blocklists = ref<FocusBlocklist[]>([]);
const activeId = ref<string | null>(null);
const active = ref(false);
const service = ref<FocusServiceStatus>({
  installed: false,
  running: false,
  healthy: false,
});
const editing = ref<FocusBlocklist | null>(null);
const formOpen = ref(false);
const name = ref("");
const domains = ref("");
const icon = ref("");
const kind = ref<"domains" | "raw">("domains");
const busy = ref(false);
const message = ref("");

const activeList = computed(() =>
  blocklists.value.find((item) => item.id === activeId.value),
);
async function refresh() {
  const [lists, state, health] = await Promise.all([
    props.client.call("getFocusBlocklists", undefined, "focus-lists"),
    props.client.call("getFocusActiveState", undefined, "focus-state"),
    props.client.call("getFocusServiceStatus", undefined, "focus-service"),
  ]);
  if (lists) blocklists.value = lists;
  if (state) {
    active.value = state.active;
    activeId.value = state.blocklist_id;
  }
  if (health) service.value = health;
}

function beginCreate() {
  formOpen.value = true;
  editing.value = null;
  name.value = "";
  domains.value = "";
  icon.value = "🛡️";
  kind.value = "domains";
}
function beginEdit(item: FocusBlocklist) {
  if (item.id === activeId.value) {
    message.value = "Нельзя изменить активный блок-лист.";
    return;
  }
  editing.value = item;
  formOpen.value = true;
  name.value = item.name;
  domains.value = item.domains.join("\n");
  icon.value = item.icon;
  kind.value = item.kind;
}
async function save() {
  if (!name.value.trim() || busy.value) return;
  busy.value = true;
  message.value = "";
  const params: FocusUpsertParams = {
    name: name.value.trim(),
    domains: parseFocusDomains(domains.value),
    icon: icon.value,
    kind: kind.value,
  };
  if (editing.value) {
    params.id = editing.value.id;
    params.preset = editing.value.preset;
  }
  const result = await props.client.call(
    "upsertFocusBlocklist",
    params,
    "focus-save",
  );
  busy.value = false;
  if (result) {
    editing.value = result;
    await refresh();
  }
}
async function remove(item: FocusBlocklist) {
  if (item.id === activeId.value || busy.value) {
    message.value = "Нельзя удалить активный блок-лист.";
    return;
  }
  busy.value = true;
  await props.client.call(
    "deleteFocusBlocklist",
    { id: item.id },
    `focus-delete-${item.id}`,
  );
  busy.value = false;
  if (editing.value?.id === item.id) editing.value = null;
  await refresh();
}
async function serviceAction(
  method:
    | "installFocusService"
    | "uninstallFocusService"
    | "startFocusService"
    | "stopFocusService",
) {
  if (busy.value) return;
  busy.value = true;
  await props.client.call(method, undefined, `focus-${method}`);
  busy.value = false;
  await refresh();
}
onMounted(() => void refresh());
</script>

<template>
  <div class="stack settings-page">
    <SettingsList>
      <SettingsRow
        title="Системная служба"
        description="Управление службой требует явного действия и может показать UAC."
      >
        <template #control>
          <StatusDot
            :tone="
              service.healthy
                ? 'success'
                : service.installed
                  ? 'warning'
                  : 'neutral'
            "
            :label="
              service.healthy
                ? 'Работает'
                : service.installed
                  ? 'Не отвечает'
                  : 'Не установлена'
            "
          />
        </template>
      </SettingsRow>
      <SettingsRow
        title="Управление службой"
        description="Установка и запуск службы фокуса"
      >
        <template #control>
          <Button
            variant="surface"
            size="sm"
            :disabled="busy"
            @click="serviceAction('installFocusService')"
            >{{ service.installed ? "Переустановить" : "Установить" }}</Button
          >
          <Button
            v-if="service.installed && !service.running"
            size="sm"
            variant="surface"
            :disabled="busy"
            @click="serviceAction('startFocusService')"
            >Запустить</Button
          >
          <Button
            v-if="service.running"
            size="sm"
            variant="surface"
            :disabled="busy"
            @click="serviceAction('stopFocusService')"
            >Остановить</Button
          >
          <Button
            v-if="service.installed"
            size="sm"
            variant="danger"
            :disabled="busy"
            @click="serviceAction('uninstallFocusService')"
            >Удалить</Button
          >
        </template>
      </SettingsRow>
    </SettingsList>

    <SettingsList>
      <SettingsRow
        title="Фокус"
        :description="
          activeList
            ? `Активен блок-лист «${activeList.name}»`
            : active
              ? `Активен блок-лист ${activeId}`
              : 'Не активен'
        "
      >
        <template #control>
          <StatusDot
            :tone="active ? 'success' : 'neutral'"
            :label="active ? 'Включено' : 'Выключено'"
          />
        </template>
      </SettingsRow>
    </SettingsList>

    <SettingsList>
      <SettingsButtonRow
        title="Блок-листы"
        description="Наборы доменов и ссылок для режима фокуса"
        button-label="Создать"
        variant="surface"
        @click="beginCreate"
      />
      <SettingsRow
        v-for="item in blocklists"
        :key="item.id"
        :title="`${item.icon} ${item.name}`"
        :description="`${item.domains.length} ${item.domains.length === 1 ? 'домен' : 'доменов'}${item.id === activeId ? ' · активен' : ''}`"
      >
        <template #control>
          <Button size="sm" variant="surface" @click="beginEdit(item)"
            >Настроить</Button
          >
          <Button
            size="sm"
            variant="danger"
            :disabled="busy"
            @click="remove(item)"
            >Удалить</Button
          >
        </template>
      </SettingsRow>
      <SettingsRow v-if="!blocklists.length" title="Блок-листов нет" />
    </SettingsList>
    <p v-if="message" class="error" role="alert">{{ message }}</p>

    <form v-if="formOpen" class="stack" @submit.prevent="save">
      <SettingsList>
        <SettingsRow
          :title="editing ? 'Изменить блок-лист' : 'Новый блок-лист'"
        >
          <template #control
            ><Button size="sm" variant="surface" type="submit" :disabled="busy"
              >Сохранить</Button
            ></template
          >
        </SettingsRow>
        <SettingsRow title="Название">
          <template #control
            ><TextInput v-model="name" placeholder="Например, Работа"
          /></template>
        </SettingsRow>
        <SettingsRow title="Иконка">
          <template #control><TextInput v-model="icon" /></template>
        </SettingsRow>
        <SettingsRow title="Домены или @ссылки">
          <template #control><Textarea v-model="domains" rows="8" /></template>
        </SettingsRow>
        <SettingsRow title="Формат">
          <template #control>
            <label class="kind"
              ><input v-model="kind" type="radio" value="domains" />
              Домены</label
            >
            <label class="kind"
              ><input v-model="kind" type="radio" value="raw" /> Ссылки</label
            >
          </template>
        </SettingsRow>
      </SettingsList>
    </form>
  </div>
</template>

<style scoped>
.kind {
  display: flex;
  gap: var(--space-1);
  align-items: center;
  color: var(--muted-foreground);
}
</style>
