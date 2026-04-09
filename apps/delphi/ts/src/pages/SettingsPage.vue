<script setup lang="ts">
import { onMounted, onUnmounted, ref, shallowRef } from "vue";
import { Database, Globe, Link, SunMoon, Unlink } from "lucide-vue-next";
import { useTheme } from "@/composables/useTheme";
import {
  arkSync,
  getArkApiKey,
  getArkUrl,
  setArkApiKey,
  setArkUrl,
} from "@/services/sync/ark-types";
import { parseConnectionString } from "@/services/sync/pairing";
import {
  type Space,
  deriveSpaceId,
  formatSpaceCode,
  getActiveSpace,
  getSpaces,
  removeSpace,
  renameSpace,
} from "@/services/space/space-manager";

const { theme, setTheme } = useTheme();

// --- Space management ---
const spaces = ref<Space[]>([]);
const activeSpaceCode = shallowRef<string | null>(null);
const renamingCode = shallowRef<string | null>(null);
const renameInput = shallowRef("");
const deletingCode = shallowRef<string | null>(null);

function startRename(space: Space) {
  renamingCode.value = space.code;
  renameInput.value = space.name;
}

async function confirmRename() {
  if (!renamingCode.value) return;
  await renameSpace(renamingCode.value, renameInput.value);
  spaces.value = await getSpaces();
  renamingCode.value = null;
}

function cancelRename() {
  renamingCode.value = null;
}

async function confirmDeleteSpace() {
  if (!deletingCode.value) return;
  const code = deletingCode.value;
  if (window.electronAPI?.invoke) {
    const spaceId = await deriveSpaceId(code);
    await window.electronAPI.invoke("db:deleteSpace", spaceId).catch(() => {});
  }
  await removeSpace(code);
  spaces.value = await getSpaces();
  deletingCode.value = null;
}

const isPaired = Boolean(getArkUrl() && getArkApiKey());
const arkConnected = shallowRef(arkSync.isConnected);
const arkMessage = shallowRef("");
const connectionCode = shallowRef("");
const paired = shallowRef(isPaired);

// Status listener with cleanup
let unsub: (() => void) | undefined;
onMounted(async () => {
  spaces.value = await getSpaces();
  activeSpaceCode.value = await getActiveSpace();
  unsub = arkSync.onStatus((connected: boolean) => {
    arkConnected.value = connected;
  });
});
onUnmounted(() => {
  unsub?.();
});

function handleConnect() {
  const parsed = parseConnectionString(connectionCode.value);
  if (!parsed) {
    arkMessage.value = "Неверный формат. Ожидается: ark://host:port?key=...";
    return;
  }

  setArkUrl(parsed.server_url);
  setArkApiKey(parsed.api_key);
  paired.value = true;

  // Auto-connect
  arkSync.disconnect();
  arkSync.connect(parsed.server_url, parsed.api_key);
  arkMessage.value = "Подключено к Ark.";
}

function handleUnpair() {
  arkSync.disconnect();
  setArkUrl("");
  setArkApiKey("");
  arkConnected.value = false;
  paired.value = false;
  arkMessage.value = "Устройство отвязано.";
}

function handleReconnectArk() {
  const url = getArkUrl();
  const key = getArkApiKey();
  if (!url || !key) return;

  arkSync.disconnect();
  arkSync.connect(url, key);
  arkMessage.value = "Подключение...";
}

function handleDisconnectArk() {
  arkSync.disconnect();
  arkConnected.value = false;
  arkMessage.value = "Отключено от Ark.";
}
</script>

<template>
  <div class="h-full min-h-0 w-full overflow-auto bg-(--background) p-4">
    <div
      class="mx-auto flex w-full max-w-(--bringhurst-wide) flex-col gap-4 py-6"
    >
      <!-- Spaces section -->
      <section
        class="bg-(--background) border-(--border) w-full rounded-xl border p-5"
      >
        <h2 class="mb-3 inline-flex items-center gap-2 text-base font-semibold">
          <Globe :size="16" />
          Пространства
        </h2>
        <p class="text-(--muted-foreground) mb-4 text-sm">
          Управление пространствами синхронизации.
        </p>

        <div
          v-if="spaces.length === 0"
          class="text-(--muted-foreground) text-sm"
        >
          Нет сохранённых пространств.
        </div>

        <div v-else class="space-y-2">
          <div
            v-for="space in spaces"
            :key="space.code"
            :class="[
              'border-(--border) flex items-center justify-between rounded-lg border px-3 py-2',
              activeSpaceCode === space.code
                ? 'border-emerald-500/40 bg-emerald-500/5'
                : '',
            ]"
          >
            <!-- Delete confirmation -->
            <template v-if="deletingCode === space.code">
              <span class="text-xs text-rose-400"
                >Удалить пространство и все данные?</span
              >
              <div class="flex gap-2">
                <button
                  class="rounded px-2 py-1 text-xs text-rose-400 transition-colors hover:bg-rose-500/10"
                  @click="confirmDeleteSpace"
                >
                  Да
                </button>
                <button
                  class="text-(--muted-foreground) rounded px-2 py-1 text-xs transition-colors hover:bg-(--muted)"
                  @click="deletingCode = null"
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
                  OK
                </button>
                <button
                  class="text-(--muted-foreground) rounded px-2 py-1 text-xs transition-colors hover:bg-(--muted)"
                  @click="cancelRename"
                >
                  Отмена
                </button>
              </div>
            </template>
            <!-- Normal view -->
            <template v-else>
              <div class="flex-1">
                <div class="flex items-center gap-2">
                  <span class="text-sm font-medium">{{ space.name }}</span>
                  <span
                    v-if="activeSpaceCode === space.code"
                    class="rounded-full bg-emerald-500/15 px-1.5 py-0.5 text-[10px] text-emerald-500"
                  >
                    активно
                  </span>
                </div>
                <span
                  class="text-(--muted-foreground) block font-mono text-[11px] tracking-wider"
                >
                  {{ formatSpaceCode(space.code) }}
                </span>
              </div>
              <div class="ml-2 flex gap-1">
                <button
                  class="text-(--muted-foreground) rounded p-1.5 text-xs transition-colors hover:bg-(--muted) hover:text-(--foreground)"
                  @click="startRename(space)"
                  title="Переименовать"
                >
                  Переименовать
                </button>
                <button
                  v-if="activeSpaceCode !== space.code"
                  class="text-(--muted-foreground) rounded p-1.5 text-xs transition-colors hover:bg-rose-500/10 hover:text-rose-400"
                  @click="deletingCode = space.code"
                  title="Удалить"
                >
                  Удалить
                </button>
              </div>
            </template>
          </div>
        </div>
      </section>

      <section
        class="bg-(--background) border-(--border) w-full rounded-xl border p-5"
      >
        <h2 class="mb-3 inline-flex items-center gap-2 text-base font-semibold">
          <Database :size="16" />
          Ark Server
        </h2>
        <p class="text-(--muted-foreground) mb-4 text-sm">
          Синхронизация между устройствами через Ark.
        </p>

        <!-- Paired state -->
        <template v-if="paired">
          <div class="mb-4 flex items-center gap-2 text-sm">
            <div
              :class="[
                'h-2 w-2 rounded-full',
                arkConnected ? 'bg-emerald-500' : 'bg-rose-500',
              ]"
            />
            <span class="text-(--muted-foreground)">
              {{ arkConnected ? "Подключено" : "Отключено" }}
            </span>
          </div>

          <label class="mb-2 block text-sm font-medium">Сервер</label>
          <input
            :value="getArkUrl()"
            readonly
            class="border-(--border) bg-(--secondary) text-(--muted-foreground) mb-4 w-full rounded-md border px-3 py-2 text-sm font-mono"
          />

          <div class="flex items-center gap-2">
            <button
              v-if="arkConnected"
              type="button"
              class="bg-(--secondary) text-(--secondary-foreground) inline-flex items-center gap-2 rounded-md px-3 py-2 text-sm"
              @click="handleDisconnectArk"
            >
              Отключиться
            </button>
            <button
              v-else
              type="button"
              class="bg-(--primary) text-(--primary-foreground) inline-flex items-center gap-2 rounded-md px-3 py-2 text-sm"
              @click="handleReconnectArk"
            >
              Подключиться
            </button>
            <button
              type="button"
              class="text-(--destructive) inline-flex items-center gap-2 rounded-md px-3 py-2 text-sm"
              @click="handleUnpair"
            >
              <Unlink :size="16" />
              Отвязать
            </button>
          </div>
        </template>

        <!-- Unpaired state -->
        <template v-else>
          <label class="mb-2 block text-sm font-medium" for="connection-code">
            Код подключения
          </label>
          <input
            id="connection-code"
            v-model="connectionCode"
            class="border-(--border) bg-(--secondary) mb-4 w-full rounded-md border px-3 py-2 text-sm font-mono"
            placeholder="ark://192.168.1.5:8000?key=..."
            autocomplete="off"
            autocapitalize="off"
            @keydown.enter="handleConnect"
          />

          <button
            type="button"
            class="bg-(--primary) text-(--primary-foreground) inline-flex items-center gap-2 rounded-md px-3 py-2 text-sm"
            @click="handleConnect"
          >
            <Link :size="16" />
            Подключить
          </button>
        </template>

        <p v-if="arkMessage" class="text-(--muted-foreground) mt-3 text-sm">
          {{ arkMessage }}
        </p>
      </section>

      <section
        class="bg-(--background) border-(--border) w-full rounded-xl border p-5"
      >
        <h2 class="mb-3 inline-flex items-center gap-2 text-base font-semibold">
          <SunMoon :size="16" />
          Theme
        </h2>
        <div class="flex flex-wrap gap-2">
          <button
            type="button"
            :class="[
              'rounded-md px-3 py-2 text-sm',
              theme === 'light'
                ? 'bg-(--primary) text-(--primary-foreground)'
                : 'bg-(--secondary)',
            ]"
            @click="setTheme('light')"
          >
            Light
          </button>
          <button
            type="button"
            :class="[
              'rounded-md px-3 py-2 text-sm',
              theme === 'dark'
                ? 'bg-(--primary) text-(--primary-foreground)'
                : 'bg-(--secondary)',
            ]"
            @click="setTheme('dark')"
          >
            Dark
          </button>
          <button
            type="button"
            :class="[
              'rounded-md px-3 py-2 text-sm',
              theme === 'system'
                ? 'bg-(--primary) text-(--primary-foreground)'
                : 'bg-(--secondary)',
            ]"
            @click="setTheme('system')"
          >
            System
          </button>
        </div>
      </section>
    </div>
  </div>
</template>
