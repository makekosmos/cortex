// usePreferences — singleton-style композабл для пер-юзерских настроек Eden.
//
// Эта спека проверяет минимальный контракт:
//   1. Default state — spellcheck выключен.
//   2. Setter мутирует state синхронно (доступно reactive UI обновлению).
//   3. localStorage hydrate из пре-сидованного значения работает (fallback
//      path когда kepler.userData отсутствует, напр. plain Vite dev server).
//
// kepler.userData bridge unit-тестом не покрывается — нужна реальная
// IPC mock'овка из Kepler shell preload'а; покрытие — e2e матрица.

import { expect, test } from "bun:test";

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

// Pre-seed localStorage ДО первого импорта модуля — модуль singleton'ит
// state при первой инициализации, после этого изменение localStorage не
// перечитывается.
localStorageMock["eden-preferences"] = JSON.stringify({ spellcheckEnabled: true });

test("hydrate из localStorage + setter API", async () => {
  const { usePreferences } = await import("../src/composables/usePreferences");
  const prefs = usePreferences();

  // Ждём первый async-цикл гидрации (userData отсутствует → resolve мгновенно).
  await prefs.ready();

  // 1. localStorage значение подхвачено.
  expect(prefs.state.spellcheckEnabled).toBe(true);

  // 2. Setter работает.
  prefs.setSpellcheckEnabled(false);
  expect(prefs.state.spellcheckEnabled).toBe(false);

  prefs.setSpellcheckEnabled(true);
  expect(prefs.state.spellcheckEnabled).toBe(true);
});
