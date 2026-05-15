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

export interface InstalledExtensionInfo {
  id: string;
  name: string;
  version: string | null;
  description: string | null;
  author: string | null;
  iconDataUri: string | null;
  backupCount: number;
  backupTimestamps: string[];
}

/** Команда в launcher'е — единица того что пользователь может вызвать. */
export interface CommandRecord {
  id: string;
  title: string;
  /** Подпись справа: имя приложения / описание категории. */
  subtitle?: string;
  /** Группа для секций в UI: 'open' = запустить апку, 'action' = ручка апки. */
  category: "open" | "action";
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
    request<T = unknown>(
      operation: string,
      params?: Record<string, unknown>,
    ): Promise<T>;
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
    installFromUrl(
      url: string,
      expectedSha256?: string | null,
    ): Promise<ExtensionInstallPreview>;
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
      set(enabled: boolean): Promise<void>;
    };
    /** Developer mode — hot reload extension'ов через Vite dev server +
        F12 для DevTools на extension window. Применяется при следующем
        открытии extension'а. */
    developerMode: {
      get(): Promise<boolean>;
      set(enabled: boolean): Promise<void>;
    };
    /** Версия Kepler из app.getVersion(). */
    version(): Promise<string>;
    /** Текущий глобальный хоткей (read-only Phase 1). */
    hotkey(): Promise<string>;
  };
}
