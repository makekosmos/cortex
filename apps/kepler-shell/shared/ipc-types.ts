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

/** Команда в launcher'е — единица того что пользователь может вызвать. */
export interface CommandRecord {
  id: string;
  title: string;
  /** Подпись справа: имя приложения / описание категории. */
  subtitle?: string;
  /** Группа для секций в UI: 'open' = запустить апку, 'action' = ручка апки. */
  category: "open" | "action";
}

export interface KeplerApi {
  /** Состояние kepler-backend подпроцесса. */
  backend: {
    status(): Promise<BackendStatus>;
    /** Перезапустить backend (если упал). */
    restart(): Promise<void>;
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
}
