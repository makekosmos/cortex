import type { KeplerApiShellServices } from "./ipc-api-shell-services";
import type { IpcJsonObject } from "./ipc-json";
// Контракт renderer API для `window.kepler` (см. preload.ts).

import type {
  CommandRecord,
  ExportConverterInfo,
  ExportResult,
  FileIndexSettings,
  FileIndexSettingsPatch,
  FileSearchDiagnosticsReport,
  FileSearchRootEstimate,
  SearchResult,
} from "./ipc-types";

// Kepler shell API contract version for `window.kepler.*`. Read as text by
// desktop/scripts/release-bom.mjs — keep the `KEPLER_API_VERSION = "x.y.z"`
// shape intact when bumping.
export const KEPLER_API_VERSION = "1.1.0";

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

  /** Поиск по ARK FTS5 через backend — пока без UI-потребителя в shell,
      оставлен для будущих использований (например, отдельный режим поиска по
      объектам через префикс или toggle). */
  search: {
    query(text: string): Promise<SearchResult[]>;
  };

  /** ARK-объекты — пока без UI-потребителя в shell. Зарезервировано на
      будущее. */
  objects: {
    listRecent(limit?: number): Promise<SearchResult[]>;
  };

  /** Generic ARK RPC bridge — используется shell-views (focus, dictation)
      для Engine ops. Main проксирует на ArkClient (см. main.ts). */
  ark: {
    request<T = unknown>(operation: string, params?: IpcJsonObject): Promise<T>;
    onEvent(listener: (event: IpcJsonObject) => void): () => void;
  };

  /** Command registry — список запуска апок + их action-ручки (Pomodoro
      start, create note и т.п.). Action-команды приходят dynamic от running
      апок через backend; static open-команды исполняются локально
      kepler-shell'ом. */
  commands: {
    list(): Promise<CommandRecord[]>;
    invoke(id: string): Promise<void>;
    /** Подписка на сигнал «список команд изменился» (апка зарегистрировала
        новые команды или вышла из эфира). Колбэк вызывается без аргументов —
        renderer'у следует заново вызвать list(). */
    onUpdated(listener: () => void): () => void;
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

  /** Shell-owned настройки индекса; сам индекс остаётся в Engine. */
  fileIndex: {
    pickRoot(): Promise<string | null>;
  };
}
