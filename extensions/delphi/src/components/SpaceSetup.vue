<script setup lang="ts">
import { onMounted, ref, shallowRef } from "vue";
import { Button, EmptyState, Modal, TextInput } from "@kosmos/visuals";
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
      color: { dark: "black", light: "white" },
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
  <Modal
    :open="true"
    :title="
      mode === 'choose'
        ? 'Пространство Ark'
        : mode === 'create'
          ? 'Пространство создано'
          : 'Присоединиться'
    "
    width="min(420px, 92vw)"
    :close-on-backdrop="false"
    hide-close
  >
    <template v-if="mode === 'choose'">
      <p class="mb-4 text-sm text-[var(--muted-foreground)]">
        Синхронизация без сервера через локальную сеть.
      </p>

      <div class="mb-4">
        <p class="mb-2 text-xs font-medium text-[var(--muted-foreground)]">
          Сохранённые пространства
        </p>

        <EmptyState
          v-if="savedSpaces.length === 0"
          compact
          title="Сохранённых пространств пока нет"
        />

        <div v-else class="space-y-2">
          <div
            v-for="space in savedSpaces"
            :key="space.code"
            class="flex items-center justify-between gap-2 rounded-lg border border-[var(--border)] px-3 py-2"
          >
            <template v-if="deletingSpace === space.code">
              <span class="text-xs text-[var(--destructive)]">Удалить с данными?</span>
              <div class="flex gap-2">
                <Button size="sm" variant="danger" @click="confirmDelete">Да</Button>
                <Button size="sm" variant="ghost" @click="cancelDelete">Нет</Button>
              </div>
            </template>

            <template v-else-if="renamingCode === space.code">
              <TextInput
                v-model="renameInput"
                class="min-w-0 flex-1"
                size="sm"
                autofocus
                @keydown.enter="confirmRename"
                @keydown.escape="cancelRename"
              />
              <div class="ml-2 flex gap-1">
                <Button size="sm" variant="primary" @click="confirmRename">Сохранить</Button>
                <Button size="sm" variant="ghost" @click="cancelRename">Отмена</Button>
              </div>
            </template>

            <template v-else>
              <button
                type="button"
                class="min-w-0 flex-1 text-left transition-colors hover:text-[var(--foreground)]"
                @click="handleRejoin(space)"
              >
                <span class="block text-sm">{{ space.name }}</span>
                <span
                  class="block font-[var(--font-mono)] text-[11px] tracking-wider text-[var(--muted-foreground)]"
                >
                  {{ formatSpaceCode(space.code) }}
                </span>
              </button>
              <div class="ml-2 flex gap-1">
                <Button size="sm" variant="ghost" @click.stop="startRename(space)">
                  Переименовать
                </Button>
                <Button size="sm" variant="danger" @click.stop="handleDelete(space)">
                  Удалить
                </Button>
              </div>
            </template>
          </div>
        </div>
      </div>

      <div class="space-y-2">
        <Button block @click="handleCreate">Создать пространство</Button>
        <Button block variant="ghost" @click="mode = 'join'">Присоединиться</Button>
      </div>
    </template>

    <template v-else-if="mode === 'create'">
      <Button class="mb-4" size="sm" variant="ghost" @click="mode = 'choose'">Назад</Button>
      <p class="mb-4 text-sm text-[var(--muted-foreground)]">
        Отсканируйте QR-код или введите код на другом устройстве.
      </p>
      <div v-if="qrDataUrl" class="mb-4 flex justify-center">
        <img :src="qrDataUrl" alt="QR-код" class="rounded-lg" width="200" height="200" />
      </div>
      <div
        class="mb-3 rounded-lg bg-[var(--muted)] p-4 text-center font-[var(--font-mono)] text-2xl tracking-widest"
      >
        {{ formatSpaceCode(generatedCode) }}
      </div>
      <Button v-if="qrPayloadRaw" class="mb-5" block variant="ghost" @click="copyLink">
        {{ copied ? "Скопировано!" : "Скопировать ссылку для подключения" }}
      </Button>
      <Button block :disabled="!generatedCode" @click="handleConfirmCreate">
        Начать использование
      </Button>
    </template>

    <template v-else>
      <Button class="mb-4" size="sm" variant="ghost" @click="mode = 'choose'">Назад</Button>
      <p class="mb-4 text-sm text-[var(--muted-foreground)]">
        Введите код пространства. Длинный код (XXXX-XXXX-XXXX-XXXX-XXX) включает адрес устройства
        для прямого подключения.
      </p>
      <TextInput
        v-model="joinInput"
        placeholder="XXXX-XXXX-XXXX или XXXX-XXXX-XXXX-XXXX-XXX"
        class="font-[var(--font-mono)] uppercase"
        :invalid="!!joinError"
        @keydown.enter="handleJoin"
      />
      <p v-if="joinError" class="mt-2 text-xs text-[var(--destructive)]">
        {{ joinError }}
      </p>
      <Button class="mt-3" block @click="handleJoin">Присоединиться</Button>
    </template>
  </Modal>
</template>
