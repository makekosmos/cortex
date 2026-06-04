<script setup lang="ts">
import { shallowRef } from "vue";
import { Button, Modal, TextInput } from "@kosmos/visuals";

const props = defineProps<{
  busy: boolean;
  errorMessage: string | null;
}>();

const emit = defineEmits<{
  submit: [connectionCode: string];
}>();

const connectionCode = shallowRef("");

function handleSubmit() {
  if (!props.busy) {
    emit("submit", connectionCode.value);
  }
}

function onKeyDown(e: KeyboardEvent) {
  if (e.key === "Enter" && !props.busy) {
    handleSubmit();
  }
}
</script>

<template>
  <Modal
    :open="true"
    title="Подключение к Ark"
    width="min(512px, 92vw)"
    :close-on-backdrop="false"
    hide-close
  >
    <p class="mb-4 text-sm text-[var(--muted-foreground)]">
      Вставьте код подключения, чтобы загрузить данные в web-режиме.
    </p>

    <label for="auth-connection-code" class="mb-2 block text-sm font-medium">
      Код подключения
    </label>
    <TextInput
      id="auth-connection-code"
      v-model="connectionCode"
      placeholder="ark://192.168.1.5:8000?key=..."
      autocomplete="off"
      :invalid="!!errorMessage"
      @keydown="onKeyDown"
    />

    <Button type="button" block class="mt-4" :loading="busy" :disabled="busy" @click="handleSubmit">
      {{ busy ? "Подключение..." : "Подключить" }}
    </Button>

    <p v-if="errorMessage" class="mt-3 text-sm text-[var(--destructive)]">
      {{ errorMessage }}
    </p>
  </Modal>
</template>
