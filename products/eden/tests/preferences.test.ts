// usePreferences — singleton-style композабл для пер-юзерских настроек Eden.
//
// Эта спека проверяет минимальный контракт:
//   1. localStorage hydrate подхватывает известные поля.
//   2. Setter'ы мутируют state синхронно (доступно reactive UI обновлению).
//   3. localStorage hydrate из пре-сидованного значения работает (fallback
//      path когда kepler.userData отсутствует, напр. plain Vite dev server).
//
// kepler.userData bridge unit-тестом не покрывается — нужна реальная
// IPC mock'овка из Kepler shell preload'а; покрытие — e2e матрица.

import { beforeEach, expect, test } from "bun:test";
import { nextTick } from "vue";

// Bun не имеет jsdom — мокаем window / localStorage минимально.
const localStorageMock: Record<string, unknown> = {};
(globalThis as unknown as { window: unknown }).window = globalThis;
(globalThis as unknown as { localStorage: Storage }).localStorage = {
  getItem: (key: string) =>
    typeof localStorageMock[key] === "string" ? (localStorageMock[key] as string) : null,
  setItem: (key: string, value: string) => {
    localStorageMock[key] = value;
  },
  removeItem: (key: string) => {
    delete localStorageMock[key];
  },
  clear: () => {
    for (const k of Object.keys(localStorageMock)) delete localStorageMock[k];
  },
  key: () => null,
  length: 0,
} as Storage;

let importSeq = 0;

function seedLocalStorage(
  snapshot: Partial<{
    spellcheckEnabled: boolean;
    readerModeEnabled: boolean;
  }>,
): void {
  localStorage.setItem("eden-preferences", JSON.stringify(snapshot));
}

async function loadFreshUsePreferences() {
  const moduleUrl = new URL(
    `../src/composables/usePreferences.ts?test=${++importSeq}`,
    import.meta.url,
  ).href;
  const { usePreferences } = await import(moduleUrl);
  return usePreferences;
}

beforeEach(() => {
  localStorage.clear();
});

test("hydrate из localStorage + setter API", async () => {
  seedLocalStorage({
    spellcheckEnabled: true,
    readerModeEnabled: true,
  });

  const usePreferences = await loadFreshUsePreferences();
  const prefs = usePreferences();

  // Ждём первый async-цикл гидрации (userData отсутствует → resolve мгновенно).
  await prefs.ready();

  // 1. localStorage значения подхвачены.
  expect(prefs.state.spellcheckEnabled).toBe(true);
  expect(prefs.state.readerModeEnabled).toBe(true);

  // 2. Setter'ы работают.
  prefs.setSpellcheckEnabled(false);
  expect(prefs.state.spellcheckEnabled).toBe(false);

  prefs.setSpellcheckEnabled(true);
  expect(prefs.state.spellcheckEnabled).toBe(true);

  prefs.setReaderModeEnabled(false);
  expect(prefs.state.readerModeEnabled).toBe(false);

  prefs.setSpellcheckEnabled(false);
  prefs.setReaderModeEnabled(false);
  expect(prefs.state.spellcheckEnabled).toBe(false);
  expect(prefs.state.readerModeEnabled).toBe(false);

  // Даем post-flush watcher'у дописать snapshot, чтобы он не протекал в следующий тест.
  await nextTick();
});

test("fresh import не наследует состояние между тестами", async () => {
  const usePreferences = await loadFreshUsePreferences();
  const prefs = usePreferences();

  await prefs.ready();

  expect(prefs.state.spellcheckEnabled).toBe(false);
  expect(prefs.state.readerModeEnabled).toBe(false);
});
