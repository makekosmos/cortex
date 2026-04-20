<template>
  <div class="settings-tab" data-testid="connected-apps-settings">
    <h1 class="settings-tab-title">Связанные программы</h1>
    <p class="settings-tab-subtitle">
      Интеграции и импорт данных из внешних приложений в Eden.
    </p>

    <div class="settings-sections">
      <section class="settings-section" data-testid="hevy-card">
        <div class="settings-section-header">
          <h2>Hevy</h2>
        </div>

        <div class="settings-section-body">
          <div class="settings-row">
            <div class="settings-row-left">
              <div class="settings-row-title">Статус подключения</div>
              <div class="settings-row-desc settings-row-desc-plain">
                Трекер тренировок — импорт тренировок и упражнений.
              </div>
            </div>
            <div class="settings-row-right">
              <span
                v-if="hevyStatus.loggedIn"
                class="badge badge-connected"
                data-testid="hevy-status-connected"
              >
                Подключено
              </span>
              <span
                v-else
                class="badge badge-disconnected"
                data-testid="hevy-status-disconnected"
              >
                Не подключено
              </span>
            </div>
          </div>

          <div class="settings-row">
            <div class="settings-row-left">
              <div class="settings-row-title">
                {{ hevyStatus.loggedIn ? "Аккаунт" : "Авторизация" }}
              </div>
              <div
                v-if="hevyStatus.loggedIn"
                class="settings-row-desc settings-row-desc-plain"
                data-testid="hevy-username"
              >
                Подключен аккаунт:
                <strong>{{ hevyStatus.username || "—" }}</strong>
              </div>
              <div v-else class="settings-row-desc settings-row-desc-plain">
                Войдите через браузер. Откроется окно hevy.com, где можно авторизоваться и дать
                Eden доступ к данным тренировок.
              </div>
            </div>
          </div>

          <div v-if="hevyStatus.loggedIn" class="settings-row-actions">
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

          <div v-else class="settings-row-actions">
            <button
              class="settings-btn-primary"
              :disabled="loginLoading"
              data-testid="hevy-login-btn"
              type="button"
              @click="handleLogin"
            >
              {{ loginLoading ? "Открываю окно..." : "Войти через Hevy" }}
            </button>
          </div>

          <div v-if="syncResult" class="settings-row">
            <div class="settings-row-left">
              <div class="sync-result" data-testid="hevy-sync-result">
                <p>
                  Добавлено тренировок: <strong>{{ syncResult.workoutsCreated }}</strong>
                </p>
                <p>
                  Пропущено как уже существующие: <strong>{{ syncResult.workoutsSkipped }}</strong>
                </p>
                <p>
                  Добавлено упражнений: <strong>{{ syncResult.exerciseEntriesCreated }}</strong>
                </p>
              </div>
            </div>
          </div>

          <div v-if="syncError" class="settings-row">
            <div class="settings-row-left">
              <div class="dialog-error" data-testid="hevy-sync-error">{{ syncError }}</div>
            </div>
          </div>

          <div v-if="loginError" class="settings-row">
            <div class="settings-row-left">
              <div class="dialog-error" data-testid="hevy-login-error">{{ loginError }}</div>
            </div>
          </div>
        </div>
      </section>
    </div>
  </div>
</template>

<script setup lang="ts">
import { onMounted, ref } from "vue";

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
    return;
  }

  loginError.value = result.error;
}

async function handleLogout() {
  if (!window.api?.hevyLogout) return;

  await window.api.hevyLogout();
  hevyStatus.value = { loggedIn: false, username: null };
  syncResult.value = null;
  syncError.value = null;
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
    return;
  }

  syncError.value = result.error;
}

onMounted(() => {
  void checkStatus();
});
</script>
