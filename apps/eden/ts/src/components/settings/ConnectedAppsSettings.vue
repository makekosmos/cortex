<template>
  <div class="settings-section" data-testid="connected-apps-settings">
    <h2>Связанные программы</h2>
    <p class="settings-description">Импорт данных из внешних приложений в Eden.</p>

    <div class="connected-app-card" data-testid="hevy-card">
      <div class="connected-app-header">
        <div class="connected-app-icon">
          <img
            src="/anytype/icon/type/default/barbell.svg"
            alt="Hevy"
            width="24"
            height="24"
            draggable="false"
          />
        </div>
        <div class="connected-app-info">
          <h3>Hevy</h3>
          <p>Трекер тренировок — импорт тренировок и упражнений</p>
        </div>
        <div class="connected-app-badge">
          <span
            v-if="hevyStatus.loggedIn"
            class="badge badge-connected"
            data-testid="hevy-status-connected"
            >Подключено</span
          >
          <span v-else class="badge badge-disconnected" data-testid="hevy-status-disconnected"
            >Не подключено</span
          >
        </div>
      </div>

      <div v-if="hevyStatus.loggedIn" class="connected-app-body">
        <p data-testid="hevy-username">
          Аккаунт: <strong>{{ hevyStatus.username || "—" }}</strong>
        </p>
        <div class="connected-app-actions">
          <button
            class="settings-btn-primary"
            :disabled="syncing"
            data-testid="hevy-sync-btn"
            type="button"
            @click="handleSync"
          >
            {{ syncing ? "Синхронизация..." : "Синхронизировать тренировки" }}
          </button>
          <button
            class="settings-btn-secondary"
            data-testid="hevy-logout-btn"
            type="button"
            @click="handleLogout"
          >
            Отключить
          </button>
        </div>
        <div v-if="syncResult" class="sync-result" data-testid="hevy-sync-result">
          <p>
            Добавлено тренировок: <strong>{{ syncResult.workoutsCreated }}</strong>
          </p>
          <p>
            Пропущено (уже есть): <strong>{{ syncResult.workoutsSkipped }}</strong>
          </p>
          <p>
            Добавлено упражнений: <strong>{{ syncResult.exerciseEntriesCreated }}</strong>
          </p>
        </div>
        <div v-if="syncError" class="dialog-error" data-testid="hevy-sync-error">
          {{ syncError }}
        </div>
      </div>

      <div v-else class="connected-app-body">
        <p>
          Войдите в аккаунт Hevy через браузер. Откроется окно hevy.com, где вы сможете
          авторизоваться.
        </p>
        <button
          class="settings-btn-primary"
          :disabled="loginLoading"
          data-testid="hevy-login-btn"
          type="button"
          @click="handleLogin"
        >
          {{ loginLoading ? "Открываю окно..." : "Войти через Hevy" }}
        </button>
        <div v-if="loginError" class="dialog-error" data-testid="hevy-login-error">
          {{ loginError }}
        </div>
      </div>
    </div>
  </div>
</template>

<script setup lang="ts">
import { ref, onMounted } from "vue";

const emit = defineEmits<{ refreshData: [] }>();

interface HevyStatus {
  loggedIn: boolean;
  username: string | null;
}
interface SyncStats {
  workoutsCreated: number;
  workoutsSkipped: number;
  exerciseEntriesCreated: number;
}

const hevyStatus = ref<HevyStatus>({ loggedIn: false, username: null });
const loginError = ref<string | null>(null);
const loginLoading = ref(false);
const syncing = ref(false);
const syncResult = ref<SyncStats | null>(null);
const syncError = ref<string | null>(null);

async function checkStatus() {
  if (!window.api?.hevyGetAuthStatus) return;
  hevyStatus.value = await window.api.hevyGetAuthStatus();
}

async function handleLogin() {
  if (!window.api?.hevyLogin) return;
  loginLoading.value = true;
  loginError.value = null;
  const result = await window.api.hevyLogin();
  loginLoading.value = false;
  if (result.ok) {
    void checkStatus();
  } else {
    loginError.value = result.error;
  }
}

async function handleLogout() {
  if (!window.api?.hevyLogout) return;
  await window.api.hevyLogout();
  hevyStatus.value = { loggedIn: false, username: null };
  syncResult.value = null;
}

async function handleSync() {
  if (!window.api?.hevySyncWorkouts) return;
  syncing.value = true;
  syncError.value = null;
  syncResult.value = null;
  const result = await window.api.hevySyncWorkouts();
  syncing.value = false;
  if (result.ok) {
    syncResult.value = result.stats;
    emit("refreshData");
  } else {
    syncError.value = result.error;
  }
}

onMounted(() => {
  void checkStatus();
});
</script>
