// usePreferences — singleton-style reactive store для пользовательских
// настроек Eden, которые не относятся к ARK-доменy (не входят в синхрон-
// изируемые объекты). Хранятся per-user через `window.kepler.userData`
// (контракт — `platform/desktop/electron/extension-host.ts`, `userData.{read,write}Json`).
//
// Поверх userData используем localStorage как fallback (на случай если
// extension открыт вне Kepler shell'а, например plain Vite dev server) —
// чтение синхронное, запись fire-and-forget.
//
// Текущие настройки:
//   - spellcheckEnabled — включает браузерный spellcheck в редакторе. По
//     умолчанию false (юзер сам жалуется на «красные подчёркивания
//     которые отвлекают»).
//   - vimModeEnabled — включает Vim motions внутри CM6-редактора.
//   - readerModeEnabled — режим чтения: системные sans-шрифты, редактирование выключено.
//
// Расширение: добавь новое поле в `EdenPreferences`, default в
// `DEFAULT_PREFERENCES`, экспортируй setter — composable сам подхватит.

import { reactive, watch } from "vue";

const PREFS_FILE_NAME = "eden-settings.json";
const LOCAL_STORAGE_KEY = "eden-preferences";

export interface EdenPreferences {
  /** false по умолчанию — браузерный spellcheck выключен. */
  spellcheckEnabled: boolean;
  /** false по умолчанию — Vim mode включается пользователем явно. */
  vimModeEnabled: boolean;
  /** false по умолчанию — Eden открывается в режиме писателя. */
  readerModeEnabled: boolean;
}

const DEFAULT_PREFERENCES: EdenPreferences = {
  spellcheckEnabled: false,
  vimModeEnabled: false,
  readerModeEnabled: false,
};

const state = reactive<EdenPreferences>({ ...DEFAULT_PREFERENCES });

let hydrated = false;
let hydratingPromise: Promise<void> | null = null;

interface KeplerUserDataBridge {
  readJson<T = unknown>(name: string): Promise<T | null>;
  writeJson(name: string, value: unknown): Promise<void>;
}

function keplerUserData(): KeplerUserDataBridge | null {
  const k = (window as unknown as { kepler?: { userData?: KeplerUserDataBridge } }).kepler;
  return k?.userData ?? null;
}

function mergeIntoState(partial: Partial<EdenPreferences> | null | undefined): void {
  if (!partial || typeof partial !== "object") return;
  if (typeof partial.spellcheckEnabled === "boolean") {
    state.spellcheckEnabled = partial.spellcheckEnabled;
  }
  if (typeof partial.vimModeEnabled === "boolean") {
    state.vimModeEnabled = partial.vimModeEnabled;
  }
  if (typeof partial.readerModeEnabled === "boolean") {
    state.readerModeEnabled = partial.readerModeEnabled;
  }
}

function readLocalStorageSync(): Partial<EdenPreferences> | null {
  try {
    const raw = localStorage.getItem(LOCAL_STORAGE_KEY);
    if (!raw) return null;
    const parsed = JSON.parse(raw) as Partial<EdenPreferences>;
    return parsed && typeof parsed === "object" ? parsed : null;
  } catch {
    return null;
  }
}

function writeLocalStorage(snapshot: EdenPreferences): void {
  try {
    localStorage.setItem(LOCAL_STORAGE_KEY, JSON.stringify(snapshot));
  } catch {
    // quota / disabled storage — silently игнорируем, userData всё равно
    // отрабатывает в Kepler shell'е.
  }
}

async function hydrate(): Promise<void> {
  if (hydrated) return;
  if (hydratingPromise) return hydratingPromise;
  hydratingPromise = (async () => {
    // 1. Pre-load из localStorage синхронно — даёт мгновенное значение пока
    //    userData round-trip идёт. Если userData потом вернёт другое — пере-
    //    запишем; рассинхрон одного frame'а UI допустим.
    mergeIntoState(readLocalStorageSync());
    // 2. Authoritative — userData (per-user, переживает clear of localStorage).
    const bridge = keplerUserData();
    if (bridge) {
      try {
        const remote = await bridge.readJson<Partial<EdenPreferences>>(PREFS_FILE_NAME);
        if (remote) mergeIntoState(remote);
      } catch (err) {
        console.warn("[eden prefs] userData.readJson failed:", err);
      }
    }
    hydrated = true;
  })();
  return hydratingPromise;
}

// Persist watcher — записываем КАЖДОЕ изменение state в userData + localStorage.
// Запускаем после первого hydrate() чтобы не перезаписать сразу defaults'ом
// загружаемое значение (был бы race: state mutate'нется в hydrate, watcher
// сработает с partially-merged value).
let watcherInstalled = false;
function installPersistWatcher(): void {
  if (watcherInstalled) return;
  watcherInstalled = true;
  watch(
    () => ({ ...state }),
    (snapshot) => {
      writeLocalStorage(snapshot);
      const bridge = keplerUserData();
      if (bridge) {
        void bridge.writeJson(PREFS_FILE_NAME, snapshot).catch((err) => {
          console.warn("[eden prefs] userData.writeJson failed:", err);
        });
      }
    },
    { deep: false, flush: "post" },
  );
}

/**
 * Возвращает реактивный объект настроек + методы. Безопасно вызывать
 * многократно — state глобальный, hydrate идемпотентен.
 *
 * Pattern: одно общее reactive() на все compose'инги — не плодим инстансы.
 */
export function usePreferences() {
  // Стартуем гидрацию (async) и установку watcher'а ровно раз. Caller'ы
  // могут читать state до конца гидрации — там будут defaults / localStorage
  // snapshot, реактивность обновит UI когда userData долетит.
  if (!hydrated && !hydratingPromise) {
    void hydrate().then(() => installPersistWatcher());
  } else if (hydrated && !watcherInstalled) {
    installPersistWatcher();
  }

  return {
    state,
    setSpellcheckEnabled(value: boolean): void {
      state.spellcheckEnabled = value;
    },
    setVimModeEnabled(value: boolean): void {
      state.vimModeEnabled = value;
    },
    setReaderModeEnabled(value: boolean): void {
      state.readerModeEnabled = value;
    },
    /** Promise, который resolve'ит когда первичный read из userData завершился. */
    ready(): Promise<void> {
      return hydratingPromise ?? Promise.resolve();
    },
  };
}
