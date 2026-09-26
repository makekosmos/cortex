// Дополнительные секции `window.kepler`, вынесенные из основного KeplerApi-контракта.

import type { StorageSummary, UpdateState } from "./ipc-types";
import type { IpcJsonObject } from "./ipc-json";

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
  gpuFeatureStatus: IpcJsonObject;
  gpuInfo: unknown;
  windows: DiagnosticsWindowInfo[];
}

interface DiagnosticsWindowMoveBenchmarkInput {
  windowKind?: "settings" | "extension" | "flatTest";
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

export interface KeplerApiShellServices {
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
    onCaptureEvent(cb: (payload: IpcJsonObject) => void): () => void;
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
    /** Repeatable setBounds benchmark for settings/extension/test windows. */
    windowMoveBenchmark(
      input?: DiagnosticsWindowMoveBenchmarkInput,
    ): Promise<DiagnosticsWindowMoveBenchmarkResult>;
  };
}
