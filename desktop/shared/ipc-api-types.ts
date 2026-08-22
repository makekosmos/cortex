import type { KeplerApiShellServices } from "./ipc-api-shell-services";
// Контракт renderer API для `window.kepler` (см. preload.ts).

import type {
  CommandRecord,
  ExportConverterInfo,
  ExportResult,
  FocusBlocklist,
  FocusSessionSnapshot,
  FocusSessionTask,
  SearchResult,
  StartFocusSessionInput,
} from "./ipc-types";

/**
 */
export interface KeplerApi extends KeplerApiShellServices {
  /** Состояние kepler-backend подпроцесса. */
  backend: {
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

  integrations: {
    connectLeetCode<T = unknown>(): Promise<T>;
    disconnectLeetCode<T = unknown>(): Promise<T>;
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

  /** Универсальный per-type data export. Phase 7. Конвертеры регистрируются
      в kepler-backend (`platform/runtime/src/export/`). Shell-only —
      external clients не получают доступ к export API. */
  export: {
    /** Список зарегистрированных converters (метадата для UI). */
    list(): Promise<ExportConverterInfo[]>;
    /** Запустить конкретный converter в указанный dest_dir. */
    run(args: { converter_id: string; format: string; dest_dir: string }): Promise<ExportResult>;
    /** Открыть native directory picker и вернуть выбранный путь
        (или null если пользователь отменил). */
    pickDir(): Promise<string | null>;
  };

  /** Shell-owned настройки индекса; сам индекс остаётся в Engine. */
  fileIndex: {
    pickRoot(): Promise<string | null>;
  };
}
