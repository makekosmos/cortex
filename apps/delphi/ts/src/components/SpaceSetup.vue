<script setup lang="ts">
import { shallowRef, watch } from "vue";
import {
  generateSpaceCode,
  formatSpaceCode,
  parseSpaceCode,
  generateQrPayload,
  parseQrPayload,
} from "@/services/space/space-manager";
import QRCode from "qrcode";

const emit = defineEmits<{
  spaceJoined: [code: string];
}>();

type Mode = "choose" | "create" | "join";
const mode = shallowRef<Mode>("choose");
const generatedCode = shallowRef("");
const joinInput = shallowRef("");
const joinError = shallowRef("");
const qrDataUrl = shallowRef("");

const isElectron = typeof window !== "undefined" && !!window.electronAPI;

async function generateQr(payload: string) {
  if (!payload) {
    qrDataUrl.value = "";
    return;
  }
  try {
    qrDataUrl.value = await QRCode.toDataURL(payload, {
      width: 200,
      margin: 2,
      color: { dark: "#000000", light: "#ffffff" },
      errorCorrectionLevel: "M",
    });
  } catch {
    qrDataUrl.value = "";
  }
}

async function handleCreate() {
  const code = generateSpaceCode();
  generatedCode.value = code;
  mode.value = "create";

  // Get own addresses via IPC for QR payload
  if (isElectron && window.electronAPI?.invoke) {
    try {
      const addresses = (await window.electronAPI.invoke(
        "sync:getOwnAddresses",
      )) as string[];
      const payload = generateQrPayload(code, addresses ?? []);
      await generateQr(payload);
    } catch {
      // Fallback: QR with just the formatted code
      await generateQr(formatSpaceCode(code));
    }
  } else {
    await generateQr(formatSpaceCode(code));
  }
}

function handleConfirmCreate() {
  emit("spaceJoined", generatedCode.value);
}

function handleJoin() {
  const input = joinInput.value.trim();

  // Try parsing as QR payload first
  const qrParsed = parseQrPayload(input);
  if (qrParsed) {
    joinError.value = "";
    emit("spaceJoined", qrParsed.code);
    return;
  }

  // Try parsing as raw code
  const parsed = parseSpaceCode(input);
  if (!parsed) {
    joinError.value =
      "Введите корректный код (XXXX-XXXX-XXXX) или ark:// ссылку";
    return;
  }
  joinError.value = "";
  emit("spaceJoined", parsed);
}
</script>

<template>
  <div
    class="fixed inset-0 z-50 flex items-center justify-center bg-black/55 p-4"
  >
    <div
      class="bg-(--background) border-(--border) w-full max-w-sm rounded-xl border p-5"
    >
      <!-- Choose mode -->
      <template v-if="mode === 'choose'">
        <h2 class="mb-1 text-lg font-semibold">Ark Space</h2>
        <p class="text-(--muted-foreground) mb-5 text-sm">
          Синхронизация без сервера — через локальную сеть. Создайте
          пространство или присоединитесь к существующему.
        </p>
        <button
          class="bg-(--foreground) text-(--background) mb-2 w-full rounded-lg px-3 py-2 text-sm font-medium transition-opacity hover:opacity-80"
          @click="handleCreate"
        >
          Создать пространство
        </button>
        <button
          class="border-(--border) w-full rounded-lg border px-3 py-2 text-sm font-medium transition-colors hover:bg-(--muted)"
          @click="mode = 'join'"
        >
          Присоединиться
        </button>
      </template>

      <!-- Create mode -->
      <template v-else-if="mode === 'create'">
        <button
          class="text-(--muted-foreground) hover:text-(--foreground) mb-4 text-xs transition-colors"
          @click="mode = 'choose'"
        >
          &larr; Назад
        </button>
        <h2 class="mb-1 text-lg font-semibold">Пространство создано</h2>
        <p class="text-(--muted-foreground) mb-4 text-sm">
          Отсканируйте QR-код или введите код на другом устройстве.
        </p>
        <div v-if="qrDataUrl" class="mb-4 flex justify-center">
          <img
            :src="qrDataUrl"
            alt="QR Code"
            class="rounded-lg"
            width="200"
            height="200"
          />
        </div>
        <div
          class="bg-(--muted) mb-5 rounded-lg p-4 text-center font-mono text-2xl tracking-widest"
        >
          {{ formatSpaceCode(generatedCode) }}
        </div>
        <button
          class="bg-(--foreground) text-(--background) w-full rounded-lg px-3 py-2 text-sm font-medium transition-opacity hover:opacity-80"
          :disabled="!generatedCode"
          @click="handleConfirmCreate"
        >
          Начать использование
        </button>
      </template>

      <!-- Join mode -->
      <template v-else>
        <button
          class="text-(--muted-foreground) hover:text-(--foreground) mb-4 text-xs transition-colors"
          @click="mode = 'choose'"
        >
          &larr; Назад
        </button>
        <h2 class="mb-1 text-lg font-semibold">Присоединиться</h2>
        <p class="text-(--muted-foreground) mb-4 text-sm">
          Введите код пространства или вставьте ark:// ссылку.
        </p>
        <input
          v-model="joinInput"
          placeholder="XXXX-XXXX-XXXX"
          class="border-(--border) bg-(--secondary) mb-1 w-full rounded-md border px-3 py-2 font-mono text-sm uppercase outline-none focus:border-(--foreground)"
          @keyup.enter="handleJoin"
        />
        <p v-if="joinError" class="mb-3 text-xs text-rose-400">
          {{ joinError }}
        </p>
        <button
          class="bg-(--foreground) text-(--background) mt-3 w-full rounded-lg px-3 py-2 text-sm font-medium transition-opacity hover:opacity-80"
          @click="handleJoin"
        >
          Присоединиться
        </button>
      </template>
    </div>
  </div>
</template>
