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
  };

  /** Поиск по ARK FTS5 через backend. Phase 1 — placeholder, реализация Phase 2+. */
  search: {
    query(text: string): Promise<SearchResult[]>;
  };
}
