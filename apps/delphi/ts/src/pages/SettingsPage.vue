<script setup lang="ts">
import { computed, onMounted, ref, shallowRef } from "vue";
import {
  CalendarDays,
  Globe,
  LogIn,
  LogOut,
  RefreshCw,
  SunMoon,
} from "lucide-vue-next";
import { useGoogleCalendar } from "@/composables/useGoogleCalendar";
import { useTheme } from "@/composables/useTheme";
import { useSidebarState } from "@/composables/useSidebarState";
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
const { wrapClass, wrapStyle } = useSidebarState();
const googleCalendar = useGoogleCalendar();
const googleActionError = shallowRef<string | null>(null);
const googleConfigMessage = shallowRef<string | null>(null);
const googleClientId = shallowRef("");
const googleClientSecret = shallowRef("");

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

function syncGoogleConfigForm() {
  if (googleCalendar.config.value.source === "env") {
    googleClientId.value = googleCalendar.config.value.clientId;
    googleClientSecret.value = "";
    return;
  }

  googleClientId.value = googleCalendar.config.value.clientId;
  googleClientSecret.value = "";
}

onMounted(async () => {
  spaces.value = await getSpaces();
  activeSpaceCode.value = await getActiveSpace();
  await googleCalendar.load();
  syncGoogleConfigForm();
});

const googleStatusMessage = computed(() => {
  if (!googleCalendar.isSupported()) {
    return "Google Calendar доступен только в Electron.";
  }

  if (!googleCalendar.status.value.configured) {
    return "Google OAuth не настроен в этой сборке.";
  }

  if (googleCalendar.status.value.lastSyncError) {
    return googleCalendar.status.value.lastSyncError;
  }

  if (googleCalendar.status.value.connected) {
    if (googleCalendar.status.value.lastSyncAt) {
      return `Последняя синхронизация: ${new Date(
        googleCalendar.status.value.lastSyncAt,
      ).toLocaleString("ru-RU")}`;
    }
    return "Аккаунт подключен.";
  }

  return "Аккаунт не подключен.";
});

const showGoogleConfigEditor = computed(
  () => googleCalendar.isSupported() && googleCalendar.config.value.source !== "env",
);

const googleConfigHint = computed(() => {
  if (!googleCalendar.isSupported()) {
    return "Google Calendar доступен только в Electron.";
  }

  if (googleCalendar.config.value.source === "env") {
    return "OAuth config приходит из переменных окружения этой сборки.";
  }

  if (googleCalendar.config.value.hasClientSecret) {
    return "Client secret уже сохранён локально. Оставьте поле пустым, чтобы не менять его.";
  }

  return "Укажите OAuth Client ID и при необходимости Client Secret для этой сборки.";
});

async function handleSaveGoogleConfig() {
  googleActionError.value = null;
  googleConfigMessage.value = null;

  try {
    if (!googleCalendar.isSupported()) {
      throw new Error("Google Calendar доступен только в Electron.");
    }

    const clientId = googleClientId.value.trim();
    if (!clientId) {
      throw new Error("Нужен Google OAuth Client ID.");
    }

    await googleCalendar.saveConfig({
      clientId,
      clientSecret: googleClientSecret.value.trim(),
    });
    syncGoogleConfigForm();
    googleConfigMessage.value = "Google OAuth config сохранён.";
  } catch (error) {
    googleActionError.value =
      error instanceof Error ? error.message : "Не удалось сохранить Google OAuth config.";
  }
}

async function handleConnectGoogle() {
  googleActionError.value = null;
  googleConfigMessage.value = null;
  try {
    await googleCalendar.load();
    syncGoogleConfigForm();

    if (!googleCalendar.isSupported()) {
      throw new Error("Google Calendar доступен только в Electron.");
    }

    if (!googleCalendar.status.value.configured) {
      throw new Error("Сначала сохраните Google OAuth Client ID в настройках ниже.");
    }

    await googleCalendar.connect();
  } catch (error) {
    googleActionError.value =
      error instanceof Error ? error.message : "Не удалось подключить Google Calendar.";
  }
}

async function handleRefreshGoogle() {
  googleActionError.value = null;
  googleConfigMessage.value = null;
  try {
    await googleCalendar.refresh(undefined, true);
  } catch (error) {
    googleActionError.value =
      error instanceof Error ? error.message : "Не удалось обновить Google Calendar.";
  }
}

async function handleDisconnectGoogle() {
  googleActionError.value = null;
  googleConfigMessage.value = null;
  try {
    await googleCalendar.disconnect();
  } catch (error) {
    googleActionError.value =
      error instanceof Error ? error.message : "Не удалось отвязать Google Calendar.";
  }
}
</script>

<template>
  <div class="h-full min-h-0 w-full overflow-auto bg-(--background) p-4">
    <div
      :class="[wrapClass, 'flex flex-col gap-4 py-6']"
      :style="wrapStyle"
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
          <CalendarDays :size="16" />
          Google Calendar
        </h2>
        <p class="text-(--muted-foreground) mb-4 text-sm">
          Просто войдите через Google. Сессия и кэш календаря сохраняются локально и переживают перезапуск приложения.
        </p>

        <div class="mt-4 flex flex-wrap items-center gap-2 text-sm">
          <span
            class="rounded-full px-2.5 py-1"
            :class="
              googleCalendar.status.value.connected
                ? 'bg-emerald-500/10 text-emerald-500'
                : 'bg-(--secondary) text-(--muted-foreground)'
            "
          >
            {{
              googleCalendar.status.value.connected
                ? 'Подключено'
                : 'Не подключено'
            }}
          </span>

          <span
            v-if="googleCalendar.account.value?.email"
            class="text-(--muted-foreground)"
          >
            {{ googleCalendar.account.value.email }}
          </span>
        </div>

        <p class="text-(--muted-foreground) mt-3 text-sm">
          {{ googleStatusMessage }}
        </p>

        <div class="mt-4 space-y-3">
          <div>
            <p class="text-(--muted-foreground) text-sm">
              {{ googleConfigHint }}
            </p>
          </div>

          <template v-if="showGoogleConfigEditor">
            <div class="space-y-2">
              <label class="block text-sm font-medium">OAuth Client ID</label>
              <input
                v-model="googleClientId"
                class="border-(--border) bg-(--secondary) w-full rounded-md border px-3 py-2 text-sm"
                autocomplete="off"
                autocapitalize="off"
                spellcheck="false"
                placeholder="1234567890-abcdef.apps.googleusercontent.com"
              />
            </div>

            <div class="space-y-2">
              <label class="block text-sm font-medium">
                OAuth Client Secret
              </label>
              <input
                v-model="googleClientSecret"
                class="border-(--border) bg-(--secondary) w-full rounded-md border px-3 py-2 text-sm"
                type="password"
                autocomplete="off"
                autocapitalize="off"
                spellcheck="false"
                :placeholder="
                  googleCalendar.config.value.hasClientSecret
                    ? 'Оставьте пустым, чтобы сохранить текущий secret'
                    : 'Опционально'
                "
              />
            </div>

            <div class="flex flex-wrap items-center gap-2">
              <button
                type="button"
                class="bg-(--secondary) text-(--secondary-foreground) inline-flex items-center gap-2 rounded-md px-3 py-2 text-sm"
                :disabled="googleCalendar.loading.value"
                @click="handleSaveGoogleConfig"
              >
                Сохранить OAuth config
              </button>
              <p
                v-if="googleConfigMessage"
                class="text-sm text-emerald-500"
              >
                {{ googleConfigMessage }}
              </p>
            </div>
          </template>
        </div>

        <p v-if="googleActionError" class="mt-2 text-sm text-rose-400">
          {{ googleActionError }}
        </p>

        <div class="mt-4 flex flex-wrap items-center gap-2">
          <button
            type="button"
            class="bg-(--primary) text-(--primary-foreground) inline-flex items-center gap-2 rounded-md px-3 py-2 text-sm"
            :disabled="googleCalendar.loading.value"
            @click="handleConnectGoogle"
          >
            <LogIn :size="16" />
            Войти через Google
          </button>

          <button
            type="button"
            class="bg-(--secondary) text-(--secondary-foreground) inline-flex items-center gap-2 rounded-md px-3 py-2 text-sm"
            :disabled="googleCalendar.loading.value || !googleCalendar.status.value.connected"
            @click="handleRefreshGoogle"
          >
            <RefreshCw
              :size="16"
              :class="googleCalendar.loading.value ? 'animate-spin' : ''"
            />
            Обновить кэш
          </button>

          <button
            type="button"
            class="text-(--destructive) inline-flex items-center gap-2 rounded-md px-3 py-2 text-sm"
            :disabled="googleCalendar.loading.value || !googleCalendar.status.value.connected"
            @click="handleDisconnectGoogle"
          >
            <LogOut :size="16" />
            Выйти
          </button>
        </div>
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
