// Контракт IPC между main и renderer для Kepler launcher.
//
// Renderer вызывает методы через `window.kepler.*` (см. preload.ts).
// Main process реализует handlers в electron/main.ts через ipcMain.handle().

export interface BackendStatus {
  running: boolean;
  pid?: number;
  wsPort?: number;
  lockFilePath: string;
}

export interface SearchResult {
  id: string;
  title: string;
  type_id: string;
  snippet?: string;
}

/**
 * @deprecated 2026-05-15: концепция spaces убрана, single DB per user.
 * Тип оставлен временно чтобы legacy consumers (если ещё импортируют) не
 * падали с typecheck-ошибкой. Удалить когда `legacy/dashboard-extension`
 * перестанет ссылаться.
 */
export interface SpaceMeta {
  id: string;
  name: string;
  objectCount: number | null;
  lastAccessedAt: number;
  label: string;
  isSelected: boolean;
}

/**
 * Manifest preview данные для install dialog'а (.kext / dir).
 * Соответствует `KextManifestPreview` из `shell/electron/extension-installer.ts`.
 */
export interface ExtensionInstallPreview {
  manifest: {
    id: string;
    name: string;
    version?: string;
    description?: string;
    author?: string;
    permissions?: string[];
    keplerApiVersion?: string;
    icon?: string;
  };
  iconDataUri: string | null;
  apiCompatError: string | null;
  isUpgrade: boolean;
  currentVersion: string | null;
}

/** Marketplace catalog entry — соответствует `CatalogExtension` из
    `shell/electron/extension-marketplace.ts`. */
export interface MarketplaceExtension {
  id: string;
  name: string;
  description: string;
  author: string;
  version: string;
  keplerApiVersion: string;
  iconUrl: string | null;
  downloadUrl: string;
  sha256: string | null;
  size: number | null;
}

export interface MarketplaceCatalog {
  schemaVersion: number;
  updatedAt: string;
  extensions: MarketplaceExtension[];
}

/** Один зарегистрированный converter в backend export registry. Phase 7. */
export interface ExportConverterInfo {
  converter_id: string;
  object_type: string;
  display_name: string;
  default_format: string;
  supported_formats: string[];
}

/** Результат `export.run` — что записано на диск и какие были ошибки. */
export interface ExportResult {
  files_written: string[];
  bytes: number;
  errors: string[];
}

export type NtfsStatus = "unknown" | "disabled" | "active" | "fallback" | "unavailable";

export interface FileIndexSettings {
  exclude_noisy_folders: boolean;
  roots: string[];
  ignore_patterns: string[];
  respect_gitignore: boolean;
  include_hidden: boolean;
  ntfs_accelerated: boolean;
  scan_in_progress: boolean;
  scan_progress: {
    phase: string;
    root: string | null;
    roots_done: number;
    roots_total: number;
    files_seen: number;
    files_indexed: number;
    message: string;
  };
  ntfs_status: NtfsStatus;
}

export interface FileIndexSettingsPatch {
  exclude_noisy_folders?: boolean;
  respect_gitignore?: boolean;
  include_hidden?: boolean;
  ntfs_accelerated?: boolean;
}

export interface InstalledExtensionInfo {
  id: string;
  name: string;
  version: string | null;
  description: string | null;
  author: string | null;
  iconDataUri: string | null;
  backupCount: number;
  backupTimestamps: string[];
  /** Источник кода extension'а:
   * - `"installed"` — user-installed в `<dataDir>/extensions/<id>/` (production flow)
   * - `"dev"` — repo dev tree (`<repoRoot>/extensions/<id>/`); auto-detect'ится
   *   когда Kepler shell запущен из репо. UI скрывает revert/uninstall кнопки
   *   для dev-source extensions (они tracked git'ом, не Kepler'ом). */
  source: "installed" | "dev";
}

/** Команда в launcher'е — единица того что пользователь может вызвать. */
export interface CommandRecord {
  id: string;
  title: string;
  /** Подпись справа: имя приложения / описание категории. */
  subtitle?: string;
  /** Группа для секций в UI: 'open' = запустить апку, 'action' = ручка апки. */
  category: "open" | "action";
  /** UI-классификация плашки. 'app' → правый лейбл «Приложение».
      'command' → «Команда · <appName>», 'file' → file-index hit.
      Если не указано — считается 'app'. */
  kind?: "app" | "command" | "file";
  /** Имя родительского приложения для command-плашек (Delphi / Horologion / Kepler). */
  appName?: string;
  /** Опциональная иконка команды. Data URI (`data:image/png;base64,...`)
      для open-команд extension'ов; undefined для action-команд. */
  icon?: string;
}

export interface KeplerApi {
  /** Состояние kepler-backend подпроцесса. */
  backend: {
    status(): Promise<BackendStatus>;
    /** Перезапустить backend (если упал). */
    restart(): Promise<void>;
    /** Subscribe на event «ArkClient handshake done, готов принимать запросы».
        Срабатывает на каждом успешном reconnect'е. Returns unsubscribe. */
    onReady(listener: () => void): () => void;
    /** Subscribe на «ArkClient disconnected» (backend упал / restart инициирован). */
    onDisconnected(listener: () => void): () => void;
  };

  /** Test rig — exposed только при `KOSMOS_TEST_MODE=1`. В production
      `window.kepler.__test` === undefined. См. tests/e2e/helpers/wait.ts. */
  __test?: {
    /** Ждёт ArkClient handshake. Резолвится сразу если уже ready. */
    waitForReady(timeoutMs?: number): Promise<void>;
    /** Snapshot текущего состояния для assertions в тестах. */
    getStats(): Promise<{
      arkConnected: boolean;
      commands: string[];
      commandsRegistered: number;
    }>;
  };

  /** Управление окном launcher'а. */
  window: {
    hide(): Promise<void>;
    /** Зарегистрировать callback на показ окна (от globalShortcut). */
    onShow(listener: () => void): () => void;
    /** Растягивает окно в expanded (с результатами) / collapsed (только input). */
    setExpanded(expanded: boolean): Promise<void>;
  };

  /** Поиск по ARK FTS5 через backend — не используется в launcher'е сейчас,
      оставлен для будущих использований (например, отдельный режим поиска по
      объектам через префикс или toggle). */
  search: {
    query(text: string): Promise<SearchResult[]>;
  };

  /** ARK-объекты — пока не показываются в launcher'е (после pivot'а на
      command registry). Зарезервировано на будущее. */
  objects: {
    listRecent(limit?: number): Promise<SearchResult[]>;
  };

  /** Generic ARK RPC bridge — используется встроенным Dashboard view'ом
      для list_object_types / list_objects / list_objects_by_type. Main
      проксирует на ArkClient (см. main.ts). */
  ark: {
    request<T = unknown>(operation: string, params?: Record<string, unknown>): Promise<T>;
  };

  /** Command registry — то что показывает launcher: список запуска апок +
      их action-ручки (Pomodoro start, create note и т.п.). Action-команды
      приходят dynamic от running апок через backend; static open-команды
      исполняются локально kepler-shell'ом. */
  commands: {
    list(): Promise<CommandRecord[]>;
    invoke(id: string): Promise<void>;
    /** Подписка на сигнал «список команд изменился» (апка зарегистрировала
        новые команды или вышла из эфира). Колбэк вызывается без аргументов —
        renderer'у следует заново вызвать list(). */
    onUpdated(listener: () => void): () => void;
  };

  /** Управление установкой / список / revert user-extensions. */
  extension: {
    /** Прочитать manifest preview из .kext или dir без extract'а. */
    installPreview(sourcePath: string): Promise<ExtensionInstallPreview>;
    /** Выполнить установку .kext / dir в `extensions/<id>/` с backup'ом. */
    installDo(sourcePath: string): Promise<ExtensionInstallPreview>;
    /** Список installed user-extensions с metadata. */
    installedList(): Promise<InstalledExtensionInfo[]>;
    /** Восстановить extension из backup'а. timestamp опционален — без него
        берётся самый свежий. Возвращает true если revert удался. */
    revert(id: string, timestamp?: string): Promise<boolean>;
    /** Список ISO-timestamp'ов доступных backup'ов для id (свежие первыми). */
    backupsList(id: string): Promise<string[]>;
    /** Удалить user copy extension'а. */
    uninstall(id: string): Promise<boolean>;
    /** Marketplace: получить catalog.json из kosmos-extensions. Cache 1h в
        main; `force=true` обходит cache. */
    catalogFetch(force?: boolean): Promise<MarketplaceCatalog>;
    /** Скачать .kext по URL и установить через existing installFromPath.
        Validate sha256 если передан. */
    installFromUrl(url: string, expectedSha256?: string | null): Promise<ExtensionInstallPreview>;
  };

  /** Универсальный per-type data export. Phase 7. Конвертеры регистрируются
      в kepler-backend (`services/kepler-backend/src/export/`). Shell-only —
      extensions не получают доступ к export API. */
  export: {
    /** Список зарегистрированных converters (метадата для UI). */
    list(): Promise<ExportConverterInfo[]>;
    /** Запустить конкретный converter в указанный dest_dir. */
    run(args: { converter_id: string; format: string; dest_dir: string }): Promise<ExportResult>;
    /** Открыть native directory picker и вернуть выбранный путь
        (или null если пользователь отменил). */
    pickDir(): Promise<string | null>;
  };

  /** Host-local File Search index settings. Storage lives in file-index.db,
      not ARK sync. */
  fileSearch: {
    settingsGet(): Promise<FileIndexSettings>;
    settingsSet(patch: FileIndexSettingsPatch): Promise<void>;
    scopeAdd(path: string): Promise<void>;
    scopeRemove(path: string): Promise<void>;
    ignoreAdd(pattern: string): Promise<void>;
    ignoreRemove(pattern: string): Promise<void>;
    rescan(): Promise<void>;
    pickScope(): Promise<string | null>;
  };

  /** Floating focus widget — Spotify-mini-player style always-on-top
      окно для активной pomodoro сессии. */
  focusWidget: {
    setState(patch: {
      active?: boolean;
      remainingSec?: number;
      label?: string;
      mode?: "work" | "break" | "stopwatch";
      blockingActive?: boolean;
      isPaused?: boolean;
    }): Promise<void>;
    getState(): Promise<{
      active: boolean;
      remainingSec: number;
      label: string;
      mode: "work" | "break" | "stopwatch";
      blockingActive: boolean;
      isPaused: boolean;
    } | null>;
    hide(): Promise<void>;
    openHorologion(): Promise<void>;
    /** Pomodoro inline controls. Прокидываются в kepler-backend
        PomodoroHost через ArkClient.request("pomodoro.<op>"). */
    pomodoro: {
      pause(): Promise<void>;
      resume(): Promise<void>;
      skip(): Promise<void>;
      stop(): Promise<void>;
    };
    /** Stopwatch (manual time_entry) stop. Закрывает running entry с
        source=manual напрямую через ARK upsert_object. */
    stopwatch: {
      stop(): Promise<void>;
    };
    /** Subscribe на push state updates от main. Returns unsubscribe. */
    onState(
      handler: (state: {
        active: boolean;
        remainingSec: number;
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
      окно через `enigo` (см. services/kepler-backend/src/dictation). */
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
     * shift, alt, win }` или `dictation_capture_cancelled` (Esc). */
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
    /** Текущий глобальный хоткей. По умолчанию `Alt+Space`. */
    hotkey(): Promise<string>;
    /** Зарегистрировать новый accelerator. Возвращает `{ok: true}` если
        OS приняла регистрацию; иначе `{ok: false, error}`. */
    hotkeySet(value: string): Promise<{ ok: boolean; error?: string }>;
    /** Сбросить хоткей в дефолт (`Alt+Space`). Возвращает применённое значение. */
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
  };
  /** Post-update first launch — main process детектит `post-update.flag` в
      userData (создаётся autoupdater-host'ом перед quitAndInstall) и шлёт
      одноразовое событие в renderer. UI показывает banner «Kepler обновлён». */
  postUpdate: {
    /** Подписка на post-update push. Returns unsubscribe. */
    onShown(listener: (payload: { version: string }) => void): () => void;
  };
}

/** autoUpdater state machine. См. shell/electron/autoupdater-host.ts. */
export type UpdateState =
  | { kind: "idle" }
  | { kind: "checking" }
  | { kind: "not-available"; checkedAt: number }
  | { kind: "available"; version: string }
  | { kind: "downloading"; version: string; percent: number }
  | { kind: "downloaded"; version: string }
  | { kind: "error"; message: string };
