<script setup lang="ts">
import { computed, onMounted, ref, watch } from "vue";
import {
  Button,
  IconButton,
  Modal,
  SettingsDropdownRow,
  SettingsList,
  SettingsRow,
  SettingsToggleRow,
  TextInput,
} from "@kosmos/visuals";
import { PhPlus } from "@phosphor-icons/vue";
import groqIcon from "../../../desktop/src/assets/providers/groq.svg";
import type { DictationConfigSnapshot } from "../manager-api";
import type { ManagerClient } from "../composables/useManagerClient";

const props = defineProps<{ client: ManagerClient }>();
const snapshot = ref<DictationConfigSnapshot | null>(null);
const modalOpen = ref(false);
const apiKey = ref("");
const message = ref("");
const verifying = ref(false);
const verified = ref(false);
const hasKey = computed(() => snapshot.value?.hasApiKey === true);
const canVerify = computed(() => apiKey.value.trim().length > 0 && !verifying.value);
const canSave = computed(() => verified.value && !verifying.value);
const providers = [{ value: "groq", label: "Groq" }];

async function refresh() {
  const next = await props.client.call<DictationConfigSnapshot>(
    "getDictationConfig",
    undefined,
    "secrets-config",
  );
  if (next) snapshot.value = next;
}
async function verify() {
  if (!apiKey.value.trim()) return;
  verifying.value = true;
  const result = await props.client.call<{ valid: boolean; message: string }>(
    "verifyDictationApiKey",
    { key: apiKey.value.trim() },
    "secrets-verify",
  );
  verifying.value = false;
  verified.value = result?.valid === true;
  if (result) message.value = result.valid ? "Ключ принят." : result.message;
}
async function save() {
  if (!canSave.value) return;
  const result = await props.client.call<{ saved: boolean }>(
    "setDictationApiKey",
    { key: apiKey.value.trim() },
    "secrets-key",
  );
  if (!result?.saved) return;
  modalOpen.value = false;
  message.value = "Ключ сохранён в Windows Credential Manager.";
  await refresh();
}
async function clear() {
  const result = await props.client.call<{ cleared: boolean }>(
    "clearDictationApiKey",
    undefined,
    "secrets-key",
  );
  if (!result?.cleared) return;
  message.value = "Ключ удалён.";
  await refresh();
}
async function setEnabled(enabled: boolean) {
  if (!snapshot.value || (!hasKey.value && enabled)) return;
  const next = await props.client.call<DictationConfigSnapshot>(
    "updateDictationConfig",
    { providerEnabled: enabled },
    "secrets-config",
  );
  if (next) snapshot.value = next;
}

watch(modalOpen, (open) => {
  if (!open) return;
  apiKey.value = "";
  message.value = "";
  verified.value = false;
});
watch(apiKey, () => (verified.value = false));
onMounted(() => void refresh());
</script>

<template>
  <section class="stack" aria-label="Ключи">
    <div class="toolbar justify-between">
      <p class="muted text-xs">
        Ключи хранятся в Windows Credential Manager.<br />
        Стоимость запросов зависит от выбранного провайдера.
      </p>
      <IconButton aria-label="Добавить ключ" :disabled="hasKey" @click="modalOpen = true">
        <PhPlus :size="16" />
      </IconButton>
    </div>
    <SettingsList>
      <SettingsRow v-if="hasKey" title="Groq" description="API ключ настроен">
        <template #leading-icon><img class="h-5 w-5" :src="groqIcon" alt="" /></template>
        <template #control>
          <Button size="sm" variant="surface" @click="clear">Удалить</Button>
        </template>
      </SettingsRow>
      <SettingsRow v-else title="Нет API ключей" muted />
      <SettingsToggleRow
        v-if="snapshot && hasKey"
        title="Использовать Groq для диктовки"
        :model-value="snapshot.config.providerEnabled"
        @update:model-value="setEnabled"
      />
    </SettingsList>
    <p v-if="message" class="muted">{{ message }}</p>

    <Modal :open="modalOpen" title="Добавить API ключ" @close="modalOpen = false">
      <div class="stack">
        <SettingsList>
          <SettingsDropdownRow title="Провайдер" model-value="groq" :options="providers" />
          <SettingsRow title="API ключ">
            <template #control>
              <TextInput
                v-model="apiKey"
                autocomplete="off"
                placeholder="Введите ключ"
              />
            </template>
          </SettingsRow>
        </SettingsList>
        <p v-if="message" class="muted">{{ message }}</p>
      </div>
      <template #footer>
        <Button
          v-if="!verified"
          size="sm"
          variant="surface"
          :disabled="!canVerify"
          :loading="verifying"
          @click="verify"
        >
          Проверить
        </Button>
        <Button v-else size="sm" variant="success" :disabled="!canSave" @click="save">Сохранить</Button>
      </template>
    </Modal>
  </section>
</template>
