import { computed, onBeforeUnmount, onMounted, reactive, shallowRef } from "vue";
import { backupApi, gamesApi, metadataApi, settingsApi } from "../../src/lib/api";
import { getAutoStartState, openPath, pickDirectoryPath, setAutoStartState } from "../../src/lib/browser";
import type { AppSettings, ArkConnectionInfo, GamesArkSyncResult } from "../../src/types";
import type { InlineFeedback } from "../components/settings/types";
import { useTheme } from "./useTheme";
import { useToast } from "./useToast";

interface SettingsFormState {
  backupDirectory: string;
  autoBackup: boolean;
  backupBeforeLaunch: boolean;
  compressionEnabled: boolean;
  compressionLevel: number;
  skipCompressionOnce: boolean;
  maxBackups: number;
  rawgApiKey: string;
}

const clampNumber = (value: number, min: number, max: number) =>
  Math.min(max, Math.max(min, value));

function applySettingsToForm(form: SettingsFormState, settings: AppSettings) {
  form.backupDirectory = settings.backup_directory;
  form.autoBackup = settings.auto_backup;
  form.backupBeforeLaunch = settings.backup_before_launch;
  form.compressionEnabled = settings.backup_compression_enabled;
  form.compressionLevel = settings.backup_compression_level;
  form.skipCompressionOnce = settings.backup_skip_compression_once;
  form.maxBackups = settings.max_backups_per_game;
  form.rawgApiKey = settings.rawg_api_key;
}

export function useSettingsPageState() {
  const { theme, setTheme } = useTheme();
  const { notify } = useToast();

  const baseSettings = shallowRef<AppSettings | null>(null);
  const loading = shallowRef(true);
  const saving = shallowRef(false);
  const autoStart = shallowRef(false);
  const autoStartPending = shallowRef(false);
  const arkConnection = shallowRef<ArkConnectionInfo | null>(null);
  const arkSyncPending = shallowRef(false);
  const arkSyncFeedback = shallowRef<InlineFeedback | null>(null);
  const arkSyncResult = shallowRef<GamesArkSyncResult | null>(null);
  const manifestRefreshing = shallowRef(false);
  const manifestFeedback = shallowRef<InlineFeedback | null>(null);
  const saveFeedback = shallowRef<InlineFeedback | null>(null);

  const form = reactive<SettingsFormState>({
    backupDirectory: "",
    autoBackup: true,
    backupBeforeLaunch: true,
    compressionEnabled: true,
    compressionLevel: 60,
    skipCompressionOnce: false,
    maxBackups: 5,
    rawgApiKey: "",
  });

  let loadRequestId = 0;
  let autoStartRequestId = 0;
  let alive = true;

  const hasSettings = computed(() => baseSettings.value !== null);
  const isDirty = computed(() => {
    if (!baseSettings.value) {
      return false;
    }

    return (
      form.backupDirectory !== baseSettings.value.backup_directory ||
      form.autoBackup !== baseSettings.value.auto_backup ||
      form.backupBeforeLaunch !== baseSettings.value.backup_before_launch ||
      form.compressionEnabled !== baseSettings.value.backup_compression_enabled ||
      form.compressionLevel !== baseSettings.value.backup_compression_level ||
      form.skipCompressionOnce !== baseSettings.value.backup_skip_compression_once ||
      form.maxBackups !== baseSettings.value.max_backups_per_game ||
      form.rawgApiKey !== baseSettings.value.rawg_api_key
    );
  });

  const saveDisabled = computed(
    () => loading.value || saving.value || !baseSettings.value || !isDirty.value,
  );

  async function loadSettings(options?: { preserveSaveFeedback?: boolean }) {
    const requestId = ++loadRequestId;
    loading.value = true;

    if (!options?.preserveSaveFeedback) {
      saveFeedback.value = null;
    }

    try {
      const [settings, autostartEnabled, connectionInfo] = await Promise.all([
        settingsApi.getAll(),
        getAutoStartState().catch((error) => {
          console.error("Failed to load autostart state:", error);
          return false;
        }),
        settingsApi.getArkConnectionInfo(),
      ]);

      if (!alive || requestId !== loadRequestId) {
        return;
      }

      baseSettings.value = settings;
      applySettingsToForm(form, settings);
      autoStart.value = autostartEnabled;
      arkConnection.value = connectionInfo;
    } catch (error) {
      if (!alive || requestId !== loadRequestId) {
        return;
      }

      console.error("Failed to load settings:", error);
      saveFeedback.value = {
        tone: "error",
        text: "Не удалось загрузить настройки.",
      };
      notify({
        tone: "error",
        title: "Не удалось загрузить настройки",
      });
    } finally {
      if (alive && requestId === loadRequestId) {
        loading.value = false;
      }
    }
  }

  async function toggleAutoStart(next?: boolean) {
    const previousState = autoStart.value;
    const newState = typeof next === "boolean" ? next : !previousState;
    const requestId = ++autoStartRequestId;

    autoStart.value = newState;
    autoStartPending.value = true;

    try {
      await setAutoStartState(newState);
    } catch (error) {
      console.error("Failed to update autostart state:", error);
      if (alive && requestId === autoStartRequestId) {
        autoStart.value = previousState;
      }
      notify({
        tone: "error",
        title: "Не удалось обновить параметр автозапуска",
      });
    } finally {
      if (alive && requestId === autoStartRequestId) {
        autoStartPending.value = false;
      }
    }
  }

  function setCompressionEnabled(next: boolean) {
    form.compressionEnabled = next;
    if (!next) {
      form.skipCompressionOnce = false;
    }
  }

  function setCompressionLevel(next: number) {
    if (Number.isNaN(next)) {
      return;
    }

    form.compressionLevel = clampNumber(next, 1, 100);
  }

  function setMaxBackups(next: number) {
    if (Number.isNaN(next)) {
      return;
    }

    form.maxBackups = clampNumber(next, 1, 100);
  }

  async function selectBackupDirectory() {
    const selectedPath = await pickDirectoryPath({
      title: "Выберите папку для резервных копий",
    });

    if (selectedPath && alive) {
      form.backupDirectory = selectedPath;
    }
  }

  async function refreshSqobaManifest() {
    if (manifestRefreshing.value) {
      return;
    }

    manifestRefreshing.value = true;
    manifestFeedback.value = null;

    try {
      await backupApi.refreshSqobaManifest();
      manifestFeedback.value = {
        tone: "success",
        text: "Манифест SQOBA обновлён.",
      };
      notify({
        tone: "success",
        title: "Манифест SQOBA обновлён",
      });
    } catch (error) {
      console.error("Failed to refresh SQOBA manifest:", error);
      manifestFeedback.value = {
        tone: "error",
        text: "Не удалось обновить манифест SQOBA.",
      };
      notify({
        tone: "error",
        title: "Не удалось обновить манифест SQOBA",
      });
    } finally {
      if (alive) {
        manifestRefreshing.value = false;
      }
    }
  }

  async function syncGamesToArk() {
    if (arkSyncPending.value) {
      return;
    }

    arkSyncPending.value = true;
    arkSyncFeedback.value = null;
    arkSyncResult.value = null;

    try {
      const result = await gamesApi.syncToArk();
      arkSyncResult.value = result;
      arkConnection.value = await settingsApi.getArkConnectionInfo();
      arkSyncFeedback.value = {
        tone: "success",
        text: "Игры синхронизированы с текущей Ark-базой.",
      };
      notify({
        tone: "success",
        title: "Игры синхронизированы с Ark",
      });
    } catch (error) {
      console.error("Failed to sync games to Ark:", error);
      arkSyncFeedback.value = {
        tone: "error",
        text: "Не удалось синхронизировать игры с Ark.",
      };
      notify({
        tone: "error",
        title: "Не удалось синхронизировать игры с Ark",
      });
    } finally {
      if (alive) {
        arkSyncPending.value = false;
      }
    }
  }

  async function openArkDatabase() {
    if (!arkConnection.value?.ark_db_path) {
      return;
    }

    await openPath(arkConnection.value.ark_db_path);
  }

  async function openArkDirectory() {
    if (!arkConnection.value?.ark_db_directory) {
      return;
    }

    await openPath(arkConnection.value.ark_db_directory);
  }

  async function saveSettings() {
    if (!baseSettings.value || saveDisabled.value) {
      return;
    }

    saving.value = true;
    saveFeedback.value = null;

    try {
      await settingsApi.update({
        ...baseSettings.value,
        backup_directory: form.backupDirectory,
        auto_backup: form.autoBackup,
        backup_before_launch: form.backupBeforeLaunch,
        backup_compression_enabled: form.compressionEnabled,
        backup_compression_level: form.compressionLevel,
        backup_skip_compression_once: form.skipCompressionOnce,
        max_backups_per_game: form.maxBackups,
        rawg_api_key: form.rawgApiKey,
        ludusavi_path: "native",
        start_minimized_in_tray: baseSettings.value.start_minimized_in_tray,
      });

      if (form.rawgApiKey !== baseSettings.value.rawg_api_key) {
        await metadataApi.setApiKey(form.rawgApiKey);
      }

      saveFeedback.value = {
        tone: "success",
        text: "Настройки сохранены.",
      };
      notify({
        tone: "success",
        title: "Настройки сохранены",
      });

      await loadSettings({ preserveSaveFeedback: true });
    } catch (error) {
      console.error("Failed to save settings:", error);
      saveFeedback.value = {
        tone: "error",
        text: "Не удалось сохранить настройки.",
      };
      notify({
        tone: "error",
        title: "Не удалось сохранить настройки",
      });
    } finally {
      if (alive) {
        saving.value = false;
      }
    }
  }

  onMounted(() => {
    void loadSettings();
  });

  onBeforeUnmount(() => {
    alive = false;
  });

  return {
    theme,
    form,
    loading,
    saving,
    hasSettings,
    autoStart,
    autoStartPending,
    arkConnection,
    arkSyncPending,
    arkSyncFeedback,
    arkSyncResult,
    manifestRefreshing,
    manifestFeedback,
    saveFeedback,
    saveDisabled,
    setTheme,
    loadSettings,
    toggleAutoStart,
    setCompressionEnabled,
    setCompressionLevel,
    setMaxBackups,
    selectBackupDirectory,
    syncGamesToArk,
    openArkDatabase,
    openArkDirectory,
    refreshSqobaManifest,
    saveSettings,
  };
}
