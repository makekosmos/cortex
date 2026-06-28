import type { KeplerApiShellServices } from "./ipc-api-shell-services";
// Контракт renderer API для `window.kepler` (см. preload.ts).

import type {
  CommandActionRequest,
  CommandActionResult,
  CommandFeedbackEvent,
  CommandFilePickerRequest,
  CommandFilePickerResult,
  CommandSnapshot,
} from "./command-ipc";
import type {
  BackendStatus,
  ClipboardHistoryItem,
  ClipboardHistorySettings,
  ClipboardHistorySettingsPatch,
  ClipboardHistoryStats,
  CommandRecord,
  ExportConverterInfo,
  ExportResult,
  FileIndexSettings,
  FileIndexSettingsPatch,
  FileSearchDiagnosticsReport,
  FileSearchRootEstimate,
  FocusBlocklist,
  FocusSessionSnapshot,
  FocusSessionTask,
  InstalledExtensionInfo,
  MarketplaceCatalog,
  SearchResult,
  StartFocusSessionInput,
} from "./ipc-types";

/**
 * Manifest preview данные для install dialog'а (.kext / dir).
 * Соответствует `KextManifestPreview` из `platform/desktop/electron/extension-installer.ts`.
 */
interface ExtensionInstallPreview {
  manifest: {
    id: string;
    name: string;
    version?: string;
    description?: string;
    author?: string;
    permissions?: string[];
    keplerApiVersion?: string;
    kind?: "vue" | "static" | "native";
    icon?: string;
    native?: {
      executable: string;
      devExecutable?: string;
      cargoPackage?: string;
      args?: string[];
      singleInstance?: boolean;
    };
  };
  iconDataUri: string | null;
  apiCompatError: string | null;
  isUpgrade: boolean;
  currentVersion: string | null;
}

export interface KeplerApi extends KeplerApiShellServices {
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

  shell: {
    openExternal(url: string): Promise<void>;
  };

  /** Управление окном launcher'а. */
  window: {
    hide(): Promise<void>;
    /** Зарегистрировать callback на показ окна (от globalShortcut). */
    onShow(listener: () => void): () => void;
    /** Зарегистрировать callback на скрытие окна. */
    onHide(listener: () => void): () => void;
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
    onEvent(listener: (event: Record<string, unknown>) => void): () => void;
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

  /** Host-local clipboard history. Хранится в instance-scoped JSON storage
      и не синхронизируется через ARK. */
  clipboardHistory: {
    list(): Promise<ClipboardHistoryItem[]>;
    copy(id: string): Promise<boolean>;
    open(id: string): Promise<boolean>;
    togglePin(id: string): Promise<boolean>;
    delete(id: string): Promise<boolean>;
    clear(): Promise<void>;
    clearAll(): Promise<void>;
    settings(): Promise<ClipboardHistorySettings>;
    updateSettings(patch: ClipboardHistorySettingsPatch): Promise<ClipboardHistorySettings>;
    stats(): Promise<ClipboardHistoryStats>;
    hide(): Promise<void>;
    onOpenShell(listener: () => void): () => void;
    onUpdated(listener: () => void): () => void;
  };

  /** Shell-owned Focus Session command page. Main process owns pomodoro,
      ARK time_entry and blocklist side effects; renderer only sends intents. */
  focusSession: {
    open(): Promise<void>;
    snapshot(): Promise<FocusSessionSnapshot>;
    listTasks(): Promise<FocusSessionTask[]>;
    listBlocklists(): Promise<FocusBlocklist[]>;
    start(input: StartFocusSessionInput): Promise<FocusSessionSnapshot>;
    pause(): Promise<FocusSessionSnapshot>;
    resume(): Promise<FocusSessionSnapshot>;
    skip(): Promise<FocusSessionSnapshot>;
    stop(): Promise<FocusSessionSnapshot>;
    /** «Выполнена»: stop + пометить привязанную задачу выполненной. */
    complete(): Promise<FocusSessionSnapshot>;
    onOpenShell(listener: () => void): () => void;
    onUpdated(listener: () => void): () => void;
    onAppBlocked(
      listener: (app: { id: string; title: string; icon?: string | null }) => void,
    ): () => void;
    snoozeApp(appId: string): Promise<void>;
  };

  /** Command host snapshots. Renderer-only read model for
      `kind:"command-extension"` view commands. */
  command: {
    snapshot(sessionId: string): Promise<CommandSnapshot | null>;
    action(sessionId: string, action: CommandActionRequest): Promise<CommandActionResult>;
    pickFiles(
      sessionId: string,
      request: CommandFilePickerRequest,
    ): Promise<CommandFilePickerResult>;
    onSnapshotUpdated(sessionId: string, listener: (snapshot: CommandSnapshot) => void): () => void;
    onFeedback(sessionId: string, listener: (event: CommandFeedbackEvent) => void): () => void;
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
    /** Marketplace: получить catalog.json из makekosmos/extensions. Cache 1h в
        main; `force=true` обходит cache. */
    catalogFetch(force?: boolean): Promise<MarketplaceCatalog>;
    /** Скачать .kext по URL и установить через existing installFromPath.
        Validate sha256 если передан. */
    installFromUrl(url: string, expectedSha256?: string | null): Promise<ExtensionInstallPreview>;
  };

  /** Универсальный per-type data export. Phase 7. Конвертеры регистрируются
      в kepler-backend (`platform/runtime/src/export/`). Shell-only —
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
    diagnostics(): Promise<FileSearchDiagnosticsReport>;
    estimateRoot(path: string): Promise<FileSearchRootEstimate>;
    scopeAdd(path: string): Promise<void>;
    scopeRemove(path: string): Promise<void>;
    ignoreAdd(pattern: string): Promise<void>;
    ignoreRemove(pattern: string): Promise<void>;
    rescan(): Promise<void>;
    clearCache(): Promise<void>;
    pickScope(): Promise<string | null>;
  };
}
