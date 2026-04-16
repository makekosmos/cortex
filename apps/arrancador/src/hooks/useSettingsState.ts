import { useCallback, useEffect, useRef, useState } from "react";
import { backupApi, metadataApi, settingsApi } from "@/lib/api";
import {
  getAutoStartState,
  pickDirectoryPath,
  setAutoStartState,
} from "@/lib/browser";
import type { AppSettings } from "@/types";

const clampNumber = (value: number, min: number, max: number) =>
  Math.min(max, Math.max(min, value));

export function useSettingsState() {
  const [settings, setSettings] = useState<AppSettings | null>(null);
  const [loading, setLoading] = useState(true);
  const [saving, setSaving] = useState(false);

  const [backupDirectory, setBackupDirectory] = useState("");
  const [autoBackup, setAutoBackup] = useState(true);
  const [backupBeforeLaunch, setBackupBeforeLaunch] = useState(true);
  const [compressionEnabled, setCompressionEnabled] = useState(true);
  const [compressionLevel, setCompressionLevel] = useState(60);
  const [skipCompressionOnce, setSkipCompressionOnce] = useState(false);
  const [maxBackups, setMaxBackups] = useState(5);
  const [rawgApiKey, setRawgApiKey] = useState("");
  const [autoStart, setAutoStart] = useState(false);
  const settingsLoadIdRef = useRef(0);
  const autostartRequestIdRef = useRef(0);
  const mountedRef = useRef(true);

  useEffect(() => {
    return () => {
      mountedRef.current = false;
    };
  }, []);

  const handleCompressionToggle = useCallback((next: boolean) => {
    setCompressionEnabled(next);
    if (!next) {
      setSkipCompressionOnce(false);
    }
  }, []);

  const handleCompressionLevelChange = useCallback((value: number) => {
    if (Number.isNaN(value)) return;
    setCompressionLevel(clampNumber(value, 1, 100));
  }, []);

  const handleMaxBackupsChange = useCallback((value: number) => {
    if (Number.isNaN(value)) return;
    setMaxBackups(clampNumber(value, 1, 100));
  }, []);

  const toggleAutoStart = useCallback(
    async (next?: boolean) => {
      const previousState = autoStart;
      const newState = typeof next === "boolean" ? next : !previousState;
      const requestId = ++autostartRequestIdRef.current;

      setAutoStart(newState);

      try {
        await setAutoStartState(newState);
      } catch (error) {
        console.error("Failed to toggle autostart:", error);
        if (mountedRef.current && requestId === autostartRequestIdRef.current) {
          setAutoStart(previousState);
        }
      }
    },
    [autoStart],
  );

  const loadSettings = useCallback(async () => {
    const requestId = ++settingsLoadIdRef.current;
    setLoading(true);
    try {
      const [appSettings, autostartEnabled] = await Promise.all([
        settingsApi.getAll(),
        getAutoStartState().catch((error) => {
          console.error("Failed to load autostart state:", error);
          return false;
        }),
      ]);

      if (!mountedRef.current || requestId !== settingsLoadIdRef.current) {
        return;
      }

      setSettings(appSettings);
      setBackupDirectory(appSettings.backup_directory);
      setAutoBackup(appSettings.auto_backup);
      setBackupBeforeLaunch(appSettings.backup_before_launch);
      setCompressionEnabled(appSettings.backup_compression_enabled);
      setCompressionLevel(appSettings.backup_compression_level);
      setSkipCompressionOnce(appSettings.backup_skip_compression_once);
      setMaxBackups(appSettings.max_backups_per_game);
      setRawgApiKey(appSettings.rawg_api_key);
      setAutoStart(autostartEnabled);
    } catch (error) {
      if (mountedRef.current && requestId === settingsLoadIdRef.current) {
        console.error("Failed to load settings:", error);
      }
    } finally {
      if (mountedRef.current && requestId === settingsLoadIdRef.current) {
        setLoading(false);
      }
    }
  }, []);

  const saveSettings = useCallback(async () => {
    if (!settings) return;
    if (mountedRef.current) {
      setSaving(true);
    }

    try {
      await settingsApi.update({
        ...settings,
        backup_directory: backupDirectory,
        auto_backup: autoBackup,
        backup_before_launch: backupBeforeLaunch,
        backup_compression_enabled: compressionEnabled,
        backup_compression_level: compressionLevel,
        backup_skip_compression_once: skipCompressionOnce,
        max_backups_per_game: maxBackups,
        rawg_api_key: rawgApiKey,
        start_minimized_in_tray: settings.start_minimized_in_tray,
        ludusavi_path: "native",
      });

      if (rawgApiKey !== settings.rawg_api_key) {
        await metadataApi.setApiKey(rawgApiKey);
      }

      await loadSettings();
    } catch (error) {
      console.error("Failed to save settings:", error);
    } finally {
      if (mountedRef.current) {
        setSaving(false);
      }
    }
  }, [
    autoBackup,
    backupBeforeLaunch,
    backupDirectory,
    compressionEnabled,
    compressionLevel,
    loadSettings,
    maxBackups,
    rawgApiKey,
    settings,
    skipCompressionOnce,
  ]);

  const selectBackupDirectory = useCallback(async () => {
    const selected = await pickDirectoryPath({
      title: "Выбрать папку для бэкапов",
    });

    if (selected && mountedRef.current) {
      setBackupDirectory(selected);
    }
  }, []);

  const refreshSqobaManifest = useCallback(async () => {
    await backupApi.refreshSqobaManifest();
  }, []);

  useEffect(() => {
    void loadSettings();
  }, [loadSettings]);

  return {
    loading,
    saving,
    backupDirectory,
    setBackupDirectory,
    autoBackup,
    setAutoBackup,
    backupBeforeLaunch,
    setBackupBeforeLaunch,
    compressionEnabled,
    handleCompressionToggle,
    compressionLevel,
    handleCompressionLevelChange,
    skipCompressionOnce,
    setSkipCompressionOnce,
    maxBackups,
    handleMaxBackupsChange,
    rawgApiKey,
    setRawgApiKey,
    autoStart,
    toggleAutoStart,
    saveSettings,
    selectBackupDirectory,
    refreshSqobaManifest,
  };
}
