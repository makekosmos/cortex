// Дополнительные секции `window.kepler`, вынесенные из основного KeplerApi-контракта.

import type { StorageSummary, SyncStatusSnapshot, UpdateState } from "./ipc-types";

interface DiagnosticsWindowInfo {
  id: number;
  title: string;
  visible: boolean;
  minimized: boolean;
  alwaysOnTop: boolean;
  bounds: { x: number; y: number; width: number; height: number };
  pid: number;
  url: string;
}

interface DiagnosticsMetricsSnapshot {
  at: string;
  appMetrics: unknown[];
  gpuFeatureStatus: Record<string, unknown>;
  gpuInfo: unknown;
  windows: DiagnosticsWindowInfo[];
}

interface DiagnosticsWindowMoveBenchmarkInput {
  windowKind?: "launcher" | "settings" | "extension" | "flatTest";
  steps?: number;
  intervalMs?: number;
  acrossDisplays?: boolean;
}

interface DiagnosticsWindowMoveBenchmarkResult {
  total_ms: number;
  intervals_p50_ms: number;
  intervals_p95_ms: number;
  intervals_max_ms: number;
  frames_over_24ms: number;
  frames_over_33ms: number;
  frames_over_50ms: number;
}

export type FocusOverlayFeedback =
  | {
      kind: "blocked";
      id: string;
      title: string;
      icon?: string | null;
    }
  | {
      kind: "completed";
      title: string;
    };

export interface KeplerApiShellServices {
  /** Floating focus widget — Spotify-mini-player style always-on-top
      окно для активной pomodoro сессии. */
  focusWidget: {
    setState(patch: {
      active?: boolean;
      remainingSec?: number;
      totalSec?: number;
      label?: string;
      mode?: "work" | "break" | "stopwatch";
      blockingActive?: boolean;
      isPaused?: boolean;
    }): Promise<void>;
    getState(): Promise<{
      active: boolean;
      remainingSec: number;
      totalSec: number;
      label: string;
      mode: "work" | "break" | "stopwatch";
      blockingActive: boolean;
      isPaused: boolean;
    } | null>;
    hide(): Promise<void>;
    openFocusSession(): Promise<void>;
    /** Pomodoro inline controls. Прокидываются в kepler-backend
        PomodoroHost через ArkClient.request("pomodoro.<op>"). */
    pomodoro: {
      pause(): Promise<void>;
      resume(): Promise<void>;
      skip(): Promise<void>;
      complete(): Promise<void>;
      /** Cancel the session without completing its task. */
      stop(): Promise<void>;
    };
    /** Stopwatch (manual time_entry) stop. Закрывает running entry с
        source=manual напрямую через ARK upsert_object. */
    stopwatch: {
      stop(): Promise<void>;
    };
    /** Показать native context menu (Редактировать / Пропустить / Скрыть). */
    showMenu(): Promise<void>;
    /** Subscribe на push state updates от main. Returns unsubscribe. */
    onState(
      handler: (state: {
        active: boolean;
        remainingSec: number;
        totalSec: number;
        label: string;
        mode: "work" | "break" | "stopwatch";
        blockingActive: boolean;
        isPaused: boolean;
      }) => void,
    ): () => void;
  };

  /** Диктация (Phase 1, Groq cloud). Main process владеет global hotkey
      регистрацией + pill window lifecycle. Audio capture происходит в
      pill renderer'е (Web Audio API → Int16 PCM → WAV → base64 → backend
      `dictation.submit_audio`). Backend инжектит транскрипт в активное
      окно через `enigo` (см. platform/runtime/src/dictation). */
  dictation: {
    /** Toggle (start ↔ stop) текущей сессии. Вызывается из global hotkey
        handler в main И из UI «Тест» кнопки в Settings. */
    toggle(): Promise<void>;
    /** Сброс текущей сессии (Esc в pill). */
    cancel(): Promise<void>;
    /** Pill renderer уведомляет main о завершении (transcript отправлен или
        ошибка) — main hide'ит окно и сбрасывает recording-флаг. */
    pillFinished(): Promise<void>;
    /** Subscribe на команды от main к pill renderer (`start` / `stop` /
        `cancel`). Pill renderer слушает и переключает audio-capture. */
    onCommand(cb: (cmd: { kind: "start" | "stop" | "cancel" }) => void): () => void;
    /** Подписка на capture events от Settings → Диктация → Горячая клавиша.
     * Backend hook ловит accelerator ниже системного уровня (это позволяет
     * назначать Win+H и др.). Events: `dictation_capture_key { vk, ctrl,
     * shift, alt, win }` (Windows) либо `{ accelerator }` (macOS — адаптер
     * резолвит mac keyCode в строку сам) или `dictation_capture_cancelled`
     * (Esc). */
    onCaptureEvent(cb: (payload: Record<string, unknown>) => void): () => void;
  };

  /** Focus mode Windows Service control. Service устанавливается опционально
      (Settings → Focus → «Установить daemon»). Когда running — hosts
      модификации идут через named pipe (no UAC). Без service — fallback
      на helper bin с UAC per toggle. */
  focusService: {
    status(): Promise<{ installed: boolean; running: boolean }>;
    ping(): Promise<boolean>;
    install(): Promise<{ ok: boolean; error?: string }>;
    uninstall(): Promise<{ ok: boolean; error?: string }>;
    start(): Promise<{ ok: boolean; error?: string }>;
    stop(): Promise<{ ok: boolean; error?: string }>;
    /** Юзер отклонил auto-install (UAC cancel). Когда true — runHelper не
        будет повторно триггерить UAC сам, install только через UI кнопку. */
    autoInstallDeclined: {
      get(): Promise<boolean>;
      set(value: boolean): Promise<void>;
    };
    /** Listener для изменений status (например, после auto-install service'а
        в runHelper). UI Settings → Фокус обновляет карточку. */
    onStatusChanged(cb: () => void): () => void;
  };

  /** Настройки Kepler (отдельное окно). Phase 1 — read-only hotkey,
      autostart toggle, версия и backend-статус (через backend.status()). */
  settings: {
    /** Открыть окно настроек (или сфокусировать существующее). */
    open(): Promise<void>;
    /** Закрыть окно настроек (вызывается из SettingsView). */
    close(): Promise<void>;
    autostart: {
      get(): Promise<boolean>;
      /** Разрешён ли autostart toggle в текущем slot'е. true только для
          prod (installed Kepler). В dev / test возвращает false — UI должен
          disable'ить toggle, потому что setLoginItemSettings из dev пишет
          мусор в HKCU Run (electron.exe из node_modules). */
      allowed(): Promise<boolean>;
      set(enabled: boolean): Promise<void>;
    };
    /** Показывать Kepler в системном трее. */
    trayIcon: {
      get(): Promise<boolean>;
      set(enabled: boolean): Promise<void>;
    };
    /** Sync state for the Settings → Sync tab. */
    sync: {
      snapshot(): Promise<SyncStatusSnapshot>;
      getPairingCode(): Promise<string | null>;
      disconnectPeer(deviceId: string): Promise<void>;
      connectWithPairingCode(code: string): Promise<void>;
      copyPairingCode(code: string): Promise<void>;
      onUpdated(listener: () => void): () => void;
    };
    /** Developer mode — hot reload extension'ов через Vite dev server +
        F12 для DevTools на extension window. Применяется при следующем
        открытии extension'а. */
    developerMode: {
      get(): Promise<boolean>;
      set(enabled: boolean): Promise<void>;
    };
    /** Трекать активные приложения (usage-tracker в kepler-backend).
        Изменения применяются после рестарта Kepler. */
    usageTracker: {
      get(): Promise<boolean>;
      set(enabled: boolean): Promise<void>;
    };
    /** Сколько минут хранить позицию в лаунчере (query / selection / scroll)
        между открытиями. 0 = всегда ресетить. Default 5. */
    launcherStateTtl: {
      get(): Promise<number>;
      set(minutes: number): Promise<void>;
    };
    /** Версия Kepler из app.getVersion(). */
    version(): Promise<string>;
    /** Сводка занимаемого места в папках данных Kosmos. */
    storageSummary(): Promise<StorageSummary>;
    /** Текущий глобальный хоткей. По умолчанию `Command+Space` на macOS и `Alt+Space` на Windows. */
    hotkey(): Promise<string>;
    /** Зарегистрировать новый accelerator. Возвращает `{ok: true}` если
        OS приняла регистрацию; иначе `{ok: false, error}`. */
    hotkeySet(value: string): Promise<{ ok: boolean; error?: string }>;
    /** Сбросить хоткей в дефолт (`Command+Space` на macOS, `Alt+Space` на Windows). Возвращает применённое значение. */
    hotkeyReset(): Promise<string>;
    /** autoUpdater control + state subscription. */
    update: {
      /** Manual trigger checkForUpdates. Returns current state после check. */
      check(): Promise<UpdateState>;
      /** quitAndInstall — клик по banner'у в downloaded состоянии. */
      install(): Promise<boolean>;
      /** Snapshot текущего state (для initial UI hydrate). */
      state(): Promise<UpdateState>;
      /** Подписка на state changes. Returns unsubscribe. */
      onStateChanged(listener: (state: UpdateState) => void): () => void;
    };
  };
  /** Eden settings window — отдельное окно поверх extension host. */
  edenSettings: {
    open(): Promise<void>;
    close(): Promise<void>;
  };
  focusOverlay: {
    ready(): void;
    onShow(listener: (feedback: FocusOverlayFeedback) => void): () => void;
    setInteractive(interactive: boolean): Promise<void>;
    showBlocked(app: { id: string; title: string; icon?: string | null }): Promise<void>;
    /** Сигнализирует main, что анимация завершена и окно можно скрыть. */
    done(): void;
  };

  /** Crash reports — locations + management для Settings → Диагностика. */
  crashes: {
    /** List files в `<data_dir>/crashes/`. Sorted newest first. */
    list(): Promise<Array<{ name: string; size: number; mtime: string }>>;
    /** Открыть директорию `<data_dir>/crashes/` в file explorer. */
    openFolder(): Promise<void>;
    /** Удалить все crash файлы. Возвращает количество удалённых. */
    clear(): Promise<{ removed: number }>;
  };
  /** Phase 4 bug-detection: diagnostics bundle для bug report'ов. */
  diagnostics: {
    /** Создать ZIP в temp dir с logs + crashes + versions + extensions. */
    bundle(): Promise<{ ok: boolean; zipPath?: string; error?: string }>;
    /** Создать ZIP + показать saveDialog. Returns final path или null
        если пользователь отменил. */
    bundleSave(): Promise<string | null>;
    /** Открыть `<data_dir>/logs/` директорию в Explorer'е. */
    openLogsFolder(): Promise<void>;
    /** Snapshot Electron process/GPU/window metrics. */
    metrics(): Promise<DiagnosticsMetricsSnapshot>;
    /** Start Chromium content trace collection. */
    traceStart(): Promise<{ ok: true }>;
    /** Stop Chromium trace collection and return the written trace path. */
    traceStop(outPath?: string): Promise<{ path: string }>;
    /** Repeatable setBounds benchmark for launcher/settings/extension/test windows. */
    windowMoveBenchmark(
      input?: DiagnosticsWindowMoveBenchmarkInput,
    ): Promise<DiagnosticsWindowMoveBenchmarkResult>;
  };
  /** Post-update first launch — main process детектит `post-update.flag` в
      userData (создаётся autoupdater-host'ом перед quitAndInstall) и шлёт
      одноразовое событие в renderer. UI показывает banner «Kepler обновлён». */
  postUpdate: {
    /** Подписка на post-update push. Returns unsubscribe. */
    onShown(listener: (payload: { version: string }) => void): () => void;
  };
}
