<script setup lang="ts">
import { onMounted, ref, shallowRef } from "vue";
import {
  type Space,
  deriveSpaceId,
  formatSpaceCode,
  generateExtendedCode,
  generateQrPayload,
  generateSpaceCode,
  getSpaces,
  parseQrPayload,
  parseSpaceCode,
  removeSpace,
  renameSpace,
} from "@/services/space/space-manager";
import QRCode from "qrcode";

const emit = defineEmits<{
  spaceJoined: [code: string, addresses: string[]];
  spaceDeleted: [code: string];
}>();

type Mode = "choose" | "create" | "join";
const mode = shallowRef<Mode>("choose");
const savedSpaces = ref<Space[]>([]);
onMounted(async () => {
  savedSpaces.value = await getSpaces();
});
const deletingSpace = shallowRef<string | null>(null);

function handleRejoin(space: Space) {
  emit("spaceJoined", space.code, []);
}

async function handleDelete(space: Space) {
  deletingSpace.value = space.code;
}

async function confirmDelete() {
  if (!deletingSpace.value) return;
  const code = deletingSpace.value;

  // Delete DB files via IPC
  if (window.electronAPI?.invoke) {
    const spaceId = await deriveSpaceId(code);
    await window.electronAPI.invoke("db:deleteSpace", spaceId).catch(() => {});
  }

  await removeSpace(code);
  savedSpaces.value = await getSpaces();
  emit("spaceDeleted", code);
  deletingSpace.value = null;
}

function cancelDelete() {
  deletingSpace.value = null;
}
// Rename state
const renamingCode = shallowRef<string | null>(null);
const renameInput = shallowRef("");

function startRename(space: Space) {
  renamingCode.value = space.code;
  renameInput.value = space.name;
}

async function confirmRename() {
  if (!renamingCode.value) return;
  await renameSpace(renamingCode.value, renameInput.value);
  savedSpaces.value = await getSpaces();
  renamingCode.value = null;
}

function cancelRename() {
  renamingCode.value = null;
}

const generatedCode = shallowRef(""); // 12-char secret
const displayCode = shallowRef(""); // 19-char extended (with IP) or 12-char fallback
const joinInput = shallowRef("");
const joinError = shallowRef("");
const qrDataUrl = shallowRef("");

const isElectron = typeof window !== "undefined" && !!window.electronAPI;
const qrPayloadRaw = shallowRef(""); // full ark://join?... for copy
const copied = shallowRef(false);

async function copyLink() {
  if (!qrPayloadRaw.value) return;
  try {
    await navigator.clipboard.writeText(qrPayloadRaw.value);
    copied.value = true;
    setTimeout(() => (copied.value = false), 2000);
  } catch {
    /* clipboard blocked */
  }
}

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
  displayCode.value = code; // fallback — may be upgraded below
  mode.value = "create";

  // Get own addresses via IPC for QR payload and extended code
  if (isElectron && window.electronAPI?.invoke) {
    try {
      const addresses = (await window.electronAPI.invoke("sync:getOwnAddresses")) as string[];
      const payload = generateQrPayload(code, addresses ?? []);
      qrPayloadRaw.value = payload;
      await generateQr(payload);

      // Build extended code from first LAN IPv4 address
      const primaryAddr = (addresses ?? []).find((a) => /^\d+\.\d+\.\d+\.\d+:\d+$/.test(a));
      if (primaryAddr) {
        const ipv4 = primaryAddr.split(":")[0];
        const ext = generateExtendedCode(code, ipv4);
        if (ext) displayCode.value = ext;
      }
    } catch {
      await generateQr(formatSpaceCode(code));
    }
  } else {
    await generateQr(formatSpaceCode(code));
  }
}

function handleConfirmCreate() {
  emit("spaceJoined", generatedCode.value, []);
}

function handleJoin() {
  const input = joinInput.value.trim();

  // Try parsing as QR payload first
  const qrParsed = parseQrPayload(input);
  if (qrParsed) {
    joinError.value = "";
    emit("spaceJoined", qrParsed.code, qrParsed.addresses);
    return;
  }

  // Try parsing as raw code
  const parsed = parseSpaceCode(input);
  if (!parsed) {
    joinError.value = "Введите корректный код (XXXX-XXXX-XXXX или XXXX-XXXX-XXXX-XXXX-XXX)";
    return;
  }
  joinError.value = "";
  emit("spaceJoined", parsed, []);
}
</script>

<template>
  <div class="fixed inset-0 z-50 flex items-center justify-center bg-black/55 p-4">
    <div class="bg-(--background) border-(--border) w-full max-w-sm rounded-xl border p-5">
      <!-- Choose mode -->
      <template v-if="mode === 'choose'">
        <h2 class="mb-1 text-lg font-semibold">Ark Space</h2>
        <p class="text-(--muted-foreground) mb-4 text-sm">
          Синхронизация без сервера — через локальную сеть.
        </p>

        <!-- Saved spaces -->
        <div v-if="savedSpaces.length > 0" class="mb-4">
          <p class="text-(--muted-foreground) mb-2 text-xs font-medium uppercase tracking-wider">
            Сохранённые пространства
          </p>
          <div class="space-y-2">
            <div
              v-for="space in savedSpaces"
              :key="space.code"
              class="border-(--border) flex items-center justify-between rounded-lg border px-3 py-2"
            >
              <!-- Delete confirmation overlay -->
              <template v-if="deletingSpace === space.code">
                <span class="text-xs text-rose-400">Удалить с данными?</span>
                <div class="flex gap-2">
                  <button
                    class="rounded px-2 py-1 text-xs text-rose-400 transition-colors hover:bg-rose-500/10"
                    @click="confirmDelete"
                  >
                    Да
                  </button>
                  <button
                    class="text-(--muted-foreground) rounded px-2 py-1 text-xs transition-colors hover:bg-(--muted)"
                    @click="cancelDelete"
                  >
                    Нет
                  </button>
                </div>
              </template>
              <!-- Rename mode -->
              <template v-else-if="renamingCode === space.code">
                <input
                  v-model="renameInput"
                  class="border-(--border) bg-(--secondary) flex-1 rounded-md border px-2 py-1 text-sm outline-none focus:border-(--foreground)"
                  @keyup.enter="confirmRename"
                  @keyup.escape="cancelRename"
                  autofocus
                />
                <div class="ml-2 flex gap-1">
                  <button
                    class="rounded px-2 py-1 text-xs text-emerald-400 transition-colors hover:bg-emerald-500/10"
                    @click="confirmRename"
                  >
                    ✓
                  </button>
                  <button
                    class="text-(--muted-foreground) rounded px-2 py-1 text-xs transition-colors hover:bg-(--muted)"
                    @click="cancelRename"
                  >
                    ✕
                  </button>
                </div>
              </template>
              <template v-else>
                <button
                  class="flex-1 text-left transition-colors hover:text-(--foreground)"
                  @click="handleRejoin(space)"
                >
                  <span class="block text-sm">{{ space.name }}</span>
                  <span
                    class="text-(--muted-foreground) block font-mono text-[11px] tracking-wider"
                  >
                    {{ formatSpaceCode(space.code) }}
                  </span>
                </button>
                <div class="ml-2 flex gap-1">
                  <button
                    class="text-(--muted-foreground) rounded p-1 text-xs transition-colors hover:text-(--foreground)"
                    @click.stop="startRename(space)"
                    title="Переименовать"
                  >
                    ✎
                  </button>
                  <button
                    class="text-(--muted-foreground) rounded p-1 text-xs transition-colors hover:text-rose-400"
                    @click.stop="handleDelete(space)"
                    title="Удалить пространство"
                  >
                    &times;
                  </button>
                </div>
              </template>
            </div>
          </div>
          <div class="border-(--border) my-4 border-t"></div>
        </div>

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
          <img :src="qrDataUrl" alt="QR Code" class="rounded-lg" width="200" height="200" />
        </div>
        <div
          class="bg-(--muted) mb-3 rounded-lg p-4 text-center font-mono text-2xl tracking-widest"
        >
          {{ formatSpaceCode(generatedCode) }}
        </div>
        <button
          v-if="qrPayloadRaw"
          class="border-(--border) mb-5 w-full rounded-lg border px-3 py-2 text-sm font-medium transition-colors hover:bg-(--muted)"
          @click="copyLink"
        >
          {{ copied ? "Скопировано!" : "Скопировать ссылку для подключения" }}
        </button>
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
          Введите код пространства. Длинный код (XXXX-XXXX-XXXX-XXXX-XXX) включает адрес устройства
          для прямого подключения.
        </p>
        <input
          v-model="joinInput"
          placeholder="XXXX-XXXX-XXXX или XXXX-XXXX-XXXX-XXXX-XXX"
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
