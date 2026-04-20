import { computed, shallowRef } from "vue";

export type Language = "ru" | "en";

type TranslationKey =
  | "sidebar.library"
  | "sidebar.scan"
  | "sidebar.catalogue"
  | "sidebar.sqoba"
  | "sidebar.statistics"
  | "sidebar.system"
  | "sidebar.settings"
  | "sidebar.achievements"
  | "sidebar.favorites"
  | "sidebar.more";

const translations: Record<Language, Record<TranslationKey, string>> = {
  ru: {
    "sidebar.library": "Библиотека",
    "sidebar.scan": "Сканирование",
    "sidebar.catalogue": "Каталог",
    "sidebar.sqoba": "SQOBA",
    "sidebar.statistics": "Статистика",
    "sidebar.system": "Система",
    "sidebar.settings": "Настройки",
    "sidebar.achievements": "Ачивки",
    "sidebar.favorites": "Избранное",
    "sidebar.more": "еще",
  },
  en: {
    "sidebar.library": "Library",
    "sidebar.scan": "Scan",
    "sidebar.catalogue": "Catalogue",
    "sidebar.sqoba": "SQOBA",
    "sidebar.statistics": "Statistics",
    "sidebar.system": "System",
    "sidebar.settings": "Settings",
    "sidebar.achievements": "Achievements",
    "sidebar.favorites": "Favorites",
    "sidebar.more": "more",
  },
};

const storageKey = "arrancador-language";
const language = shallowRef<Language>("ru");

export function initializeLanguage() {
  if (typeof window === "undefined") {
    return;
  }

  const stored = window.localStorage.getItem(storageKey);
  language.value = stored === "en" ? "en" : "ru";
  document.documentElement.lang = language.value;
}

export function useLanguage() {
  const setLanguage = (nextLanguage: Language) => {
    language.value = nextLanguage;
    if (typeof window !== "undefined") {
      window.localStorage.setItem(storageKey, nextLanguage);
      document.documentElement.lang = nextLanguage;
    }
  };

  return {
    language: computed(() => language.value),
    setLanguage,
    t: (key: TranslationKey) => translations[language.value][key] ?? key,
  };
}
