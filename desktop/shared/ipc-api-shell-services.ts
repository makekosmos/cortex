// Дополнительные секции `window.kepler`, вынесенные из основного KeplerApi-контракта.

import type { StorageSummary, UpdateState } from "./ipc-types";

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
  windowKind?: "launcher" | "settings" | "flatTest";
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

  focusService: {
    status(): Promise<{ installed: boolean; running: boolean }>;
    ping(): Promise<boolean>;
    install(): Promise<{ ok: boolean; error?: string }>;
    uninstall(): Promise<{ ok: boolean; error?: string }>;
    start(): Promise<{ ok: boolean; error?: string }>;
    stop(): Promise<{ ok: boolean; error?: string }>;
    autoInstallDeclined: {
      get(): Promise<boolean>;
      set(value: boolean): Promise<void>;
    };
    onStatusChanged(cb: () => void): () => void;
  };

  /** Настройки Kepler (отдельное окно). Shell preferences and updater only. */
  settings: {
    /** Открыть окно настроек (или сфокусировать существующее). */
    open(): Promise<void>;
    /** Закрыть окно настроек (вызывается из SettingsView). */
    close(): Promise<void>;
    /** Показывать Kepler в системном трее. */
    trayIcon: {
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
  focusOverlay: {
    ready(): void;
    onShow(listener: (feedback: FocusOverlayFeedback) => void): () => void;
    setInteractive(interactive: boolean): Promise<void>;
    showBlocked(app: { id: string; title: string; icon?: string | null }): Promise<void>;
    /** Сигнализирует main, что анимация завершена и окно можно скрыть. */
    done(): void;
  };

  /** Phase 4 bug-detection: diagnostics bundle для bug report'ов. */
  diagnostics: {
    /** Создать ZIP в temp dir с logs + crashes + versions. */
    bundle(): Promise<{ ok: boolean; zipPath?: string; error?: string }>;
    /** Snapshot Electron process/GPU/window metrics. */
    metrics(): Promise<DiagnosticsMetricsSnapshot>;
    /** Start Chromium content trace collection. */
    traceStart(): Promise<{ ok: true }>;
    /** Stop Chromium trace collection and return the written trace path. */
    traceStop(outPath?: string): Promise<{ path: string }>;
    /** Repeatable setBounds benchmark for launcher/settings/test windows. */
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
