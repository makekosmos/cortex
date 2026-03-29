<script setup lang="ts">
import { shallowRef, onMounted, onUnmounted } from "vue";
import { Database, Link, SunMoon, Unlink } from "lucide-vue-next";
import { useTheme } from "@/composables/useTheme";
import {
  arkSync,
  getArkApiKey,
  getArkUrl,
  setArkApiKey,
  setArkUrl,
} from "@/services/sync/ark-client";
import { parseConnectionString } from "@/services/sync/pairing";

const { theme, setTheme } = useTheme();

const isPaired = Boolean(getArkUrl() && getArkApiKey());
const arkConnected = shallowRef(arkSync.isConnected);
const arkMessage = shallowRef("");
const connectionCode = shallowRef("");
const paired = shallowRef(isPaired);

// Status listener with cleanup
let unsub: (() => void) | undefined;
onMounted(() => {
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
    <div class="mx-auto flex w-full max-w-lg flex-col gap-4 py-6">
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
