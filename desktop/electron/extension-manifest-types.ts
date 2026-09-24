import type { JsonRecord } from "./extension-permissions";

export type ExtensionKind = "vue" | "static" | "native";

/**
 * Объявление команды в `manifest.json` extension'а. Полный id рендерится
 * как `${extension.id}:${command.id}`.
 */
interface KextManifestCommand {
  /** Локальный id (без префикса extension'а). [a-z0-9:-]+. */
  id: string;
  /** Подпись в launcher'е (RU). */
  title: string;
  /** Доп. подпись справа (название extension'а или категория). */
  subtitle?: string;
  /** Иконка. Относительный путь от extension dir (например `icons/today.svg`).
      Если не задано — используется top-level `manifest.icon`. */
  icon?: string;
  /** Hash-route для openExtension. Передаётся в Eden через
      `kepler.navigation.initialRoute` / `onNavigate`. */
  route?: string;
  /** UI-классификация плашки. `app` = главное приложение, `command` = команда.
      По умолчанию `command`. */
  kind?: "app" | "command";
  /** Поведение при invoke:
   *  - `open` (default): открыть extension с route. Не требует running state.
   *  - `action`: invoke в running extension через ARK commands bus. Если
   *    extension не запущен — Kepler auto-launch'ит и dispatch'ит после
   *    mount. */
  mode?: "open" | "action";
}

export interface ExtensionManifest {
  id: string;
  /** Immutable application identity used by update registries. */
  appId?: string;
  name: string;
  /**
   * Собственная версия extension'а (semver `MAJOR.MINOR.PATCH`). Показывается
   * в UI install dialog'а и Settings -> Расширения. Используется backup-системой
   * (`extensions-backups/<id>/<timestamp>/`) для отката на прошлую версию.
   */
  version?: string;
  /**
   * Краткое описание extension'а (одна строка). Показывается в install dialog.
   */
  description?: string;
  /**
   * Автор / организация. Только информационно — code-signing нет.
   */
  author?: string;
  /**
   * Декларируемые permissions. Для user-installed extension'ов runtime
   * enforce'ит их в main-process IPC; repo-dev и bundled first-party copies
   * считаются trusted, чтобы встроенные приложения не дублировали broad caps.
   */
  permissions?: string[];
  /**
   * Semver-range той версии Kepler API, на которой extension работает.
   * Например `"^1.0.0"` — accept любые 1.x.y версии, отвергнуть 2.0.0.
   * Если поле отсутствует — extension считается legacy и грузится без
   * проверки (warning в console). Рекомендуется всегда указывать.
   *
   * См. [`KEPLER_API_VERSION`](./kepler-api.ts) — текущая версия shell'а.
   */
  keplerApiVersion?: string;
  kind?: ExtensionKind;
  entryHtml?: string;
  /**
   * Native extension entrypoint. Used only when `kind: "native"`.
   *
   * `executable` is relative to extension dir for installed/bundled copies.
   * `devExecutable` is optional and used from repo dev tree when it exists
   * (for example `../../target/debug/my-native-app.exe`).
   * `args` are appended before shell-provided metadata args.
   */
  native?: {
    executable: string;
    devExecutable?: string;
    cargoPackage?: string;
    args?: string[];
    singleInstance?: boolean;
  };
  /**
   * Путь к preload-скрипту:
   *  - для "vue" по умолчанию используется shared preload из dist-electron;
   *  - для "static" значение интерпретируется относительно директории
   *    extension'а (legacy PoC mode).
   *  - специальное значение "kepler-extension-preload.mjs" принудительно
   *    использует shared preload.
   */
  preload?: string;
  /**
   * Относительный путь к иконке extension'а внутри его директории
   * (обычно `icon.png`). Используется launcher'ом для open-команд.
   */
  icon?: string;
  /**
   * Порт Vite dev server'а в developer mode. Если задан и активирован
   * developer mode (env `KEPLER_DEV=1` или toggle в Settings) — extension
   * грузится с `http://localhost:<devPort>/` вместо `dist/index.html`.
   */
  devPort?: number;
  width?: number;
  height?: number;
  minWidth?: number;
  minHeight?: number;
  /**
   * Windows backdrop material для окна. Если задан, BrowserWindow создаётся
   * с прозрачным backgroundColor и Win32 system backdrop. Renderer должен
   * иметь semi-transparent body, иначе эффект не виден.
   *
   * - `"acrylic"` — blur с лёгкой прозрачностью (Win11/10)
   * - `"mica"` — desktop tint Win11
   * - `"none"` — стандартный непрозрачный фон (по умолчанию)
   */
  windowEffect?: "acrylic" | "mica" | "none";
  /**
   * Если `true` — клик на «X» / Alt+F4 / `kepler:extension:window:close`
   * **скрывает** окно вместо destroy. Renderer остаётся жив (вместе со всеми
   * таймерами, ARK подписками и side-effect'ами), reopen через `openExtension`
   * мгновенно показывает hidden окно. Используется для extensions с долго
   * живущим процессом. Окно реально уничтожается только на quit приложения.
   *
   * Default: false (классический destroy on close).
   *
   * NB: окно ВСЁ ЕЩЁ показывается / прячется в taskbar при hide — пользователь
   * видит что extension "закрыт"; индикатор того что pomodoro продолжается —
   * floating focus widget.
   */
  keepAliveInBackground?: boolean;
  /**
   * Opt-in к `backgroundThrottling: false` для extension-owned renderer'а.
   *
   * По умолчанию (`false`) Chromium throttle'ит фоновые timers/rAF — безопасный
   * baseline, hidden/свёрнутые extension windows не жгут CPU попусту.
   *
   * `true` разрешается только extensions, чья логика **обоснованно** зависит
   * от foreground-accurate timers в скрытом окне (например, real-time sync-loop
   * без backend). Фоновые side-effect'ы лучше держать в backend/main/service.
   *
   * Default: false (throttling включён).
   */
  backgroundExecution?: boolean;
  /**
   * Declarative commands extension'а (Raycast-style). Manifest = source of
   * truth для entry-point команд: открыть extension с конкретным route,
   * либо триггернуть action который extension обработает через
   * `kepler.navigation.onNavigate`. Видны в launcher всегда (пока
   * extension установлен), не требуют running state.
   *
   * Полный id команды = `${manifest.id}:${cmd.id}` — security boundary
   * (extension не может claim'нуть чужой namespace, например `settings:open`).
   *
   * Подробнее — `docs-site/concepts/command-bus.md`.
   */
  commands?: KextManifestCommand[];
  /**
   * Test contract — опционально. Используется universal `tests/e2e/extensions-contract.spec.ts`
   * чтобы автоматически проверять архитектурный baseline extension'а: команды
   * appear в `commands.list` после boot'а, ARK smoke round-trip по объявленному
   * object type работает. Per-app UI flow'ы — отдельные spec'и.
   */
  tests?: {
    /**
     * Список command id, которые extension обязан зарегистрировать через
     * `commands.register` к моменту первого render. Universal contract spec
     * после boot'а делает `commands.list` и сравнивает.
     */
    commands?: string[];
    /**
     * ARK round-trip smoke: type id + (опционально) sample payload. Spec
     * делает `upsert_object` -> `get_object` -> `delete_object`.
     */
    smoke?: {
      objectType: string;
      sample?: {
        title?: string;
        content?: unknown;
        props?: JsonRecord;
      };
    };
  };
}
