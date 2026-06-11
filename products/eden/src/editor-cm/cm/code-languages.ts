// Портировано из ZenNotes (MIT, © 2026 Adib Hanna and ZenNotes contributors), адаптировано для Eden.
/**
 * Lazy code-fence language registry for the markdown editor.
 *
 * Упрощено против оригинала: не тащим 15 пакетов @codemirror/lang-*,
 * используем только `languages` из @codemirror/language-data (lazy
 * LanguageDescription). Все языки загружаются по требованию через
 * динамические импорты, что уменьшает начальный бандл.
 *
 * Экспортирует функцию `resolveCodeLanguage` пригодную для опции
 * `codeLanguages` у `markdown()` из @codemirror/lang-markdown.
 */
import { LanguageDescription } from "@codemirror/language";
import { languages as lazyLanguages } from "@codemirror/language-data";

function normalize(info: string): string {
  return info
    .trim()
    .toLowerCase()
    .replace(/[^a-z0-9+#]/g, "");
}

/**
 * Resolver function passed to `markdown({ codeLanguages: … })`. Returns
 * a `LanguageDescription` from `@codemirror/language-data` lazy list,
 * or `null` when no grammar is available.
 */
export function resolveCodeLanguage(info: string): LanguageDescription | null {
  const key = normalize(info);
  if (!key) return null;
  return LanguageDescription.matchLanguageName(lazyLanguages, key, true);
}
