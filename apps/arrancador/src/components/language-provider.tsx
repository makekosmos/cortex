import {
  createContext,
  useContext,
  useEffect,
  useMemo,
  useState,
} from "react";

export type Language = "ru" | "en";

type TranslationKey =
  | "sidebar.library"
  | "sidebar.scan"
  | "sidebar.catalogue"
  | "sidebar.sqoba"
  | "sidebar.statistics"
  | "sidebar.system"
  | "sidebar.settings"
  | "sidebar.favorites"
  | "sidebar.more"
  | "sidebar.expand"
  | "sidebar.collapse"
  | "settings.language"
  | "settings.languageRu"
  | "settings.languageEn";

const translations: Record<Language, Record<TranslationKey, string>> = {
  ru: {
    "sidebar.library": "Библиотека",
    "sidebar.scan": "Сканирование",
    "sidebar.catalogue": "Каталог",
    "sidebar.sqoba": "SQOBA",
    "sidebar.statistics": "Статистика",
    "sidebar.system": "Система",
    "sidebar.settings": "Настройки",
    "sidebar.favorites": "Избранное",
    "sidebar.more": "ещё",
    "sidebar.expand": "Развернуть",
    "sidebar.collapse": "Свернуть",
    "settings.language": "Язык интерфейса",
    "settings.languageRu": "Русский",
    "settings.languageEn": "English",
  },
  en: {
    "sidebar.library": "Library",
    "sidebar.scan": "Scan",
    "sidebar.catalogue": "Catalogue",
    "sidebar.sqoba": "SQOBA",
    "sidebar.statistics": "Statistics",
    "sidebar.system": "System",
    "sidebar.settings": "Settings",
    "sidebar.favorites": "Favorites",
    "sidebar.more": "more",
    "sidebar.expand": "Expand",
    "sidebar.collapse": "Collapse",
    "settings.language": "Language",
    "settings.languageRu": "Russian",
    "settings.languageEn": "English",
  },
};

type LanguageContextState = {
  language: Language;
  setLanguage: (language: Language) => void;
  t: (key: TranslationKey) => string;
};

const STORAGE_KEY = "arrancador-language";

const defaultState: LanguageContextState = {
  language: "ru",
  setLanguage: () => {},
  t: (key) => translations.ru[key] ?? key,
};

const LanguageContext = createContext<LanguageContextState | undefined>(
  undefined,
);

export function LanguageProvider({ children }: { children: React.ReactNode }) {
  const [language, setLanguageState] = useState<Language>(() => {
    const stored =
      typeof localStorage !== "undefined"
        ? localStorage.getItem(STORAGE_KEY)
        : null;
    return stored === "en" ? "en" : "ru";
  });

  const setLanguage = (value: Language) => {
    localStorage.setItem(STORAGE_KEY, value);
    setLanguageState(value);
  };

  useEffect(() => {
    document.documentElement.lang = language;
  }, [language]);

  const value = useMemo<LanguageContextState>(
    () => ({
      language,
      setLanguage,
      t: (key) => translations[language][key] ?? key,
    }),
    [language],
  );

  return (
    <LanguageContext.Provider value={value}>{children}</LanguageContext.Provider>
  );
}

export function useLanguage() {
  const context = useContext(LanguageContext);
  return context ?? defaultState;
}
