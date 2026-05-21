<script setup lang="ts">
import { shallowRef } from "vue";

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
  <div class="fixed inset-0 z-50 flex items-center justify-center bg-black/55 p-4">
    <div class="bg-(--background) border-(--border) w-full max-w-lg rounded-xl border p-5">
      <h2 class="mb-2 text-lg font-semibold">Подключение к Ark</h2>
      <p class="text-(--muted-foreground) mb-4 text-sm">
        Вставьте код подключения, чтобы загрузить данные в web-режиме.
      </p>

      <label for="auth-connection-code" class="mb-2 block text-sm font-medium">
        Код подключения
      </label>
      <input
        id="auth-connection-code"
        :value="connectionCode"
        class="border-(--border) bg-(--secondary) mb-4 w-full rounded-md border px-3 py-2 text-sm"
        placeholder="ark://192.168.1.5:8000?key=..."
        autocomplete="off"
        @input="connectionCode = ($event.target as HTMLInputElement).value"
        @keydown="onKeyDown"
      />

      <button
        type="button"
        :disabled="busy"
        class="bg-(--primary) text-(--primary-foreground) w-full rounded-md px-3 py-2 text-sm disabled:opacity-60"
        @click="handleSubmit"
      >
        {{ busy ? "Подключение..." : "Подключить" }}
      </button>

      <p v-if="errorMessage" class="text-(--destructive) mt-3 text-sm">
        {{ errorMessage }}
      </p>
    </div>
  </div>
</template>
