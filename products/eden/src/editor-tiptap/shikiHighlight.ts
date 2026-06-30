import { createHighlighter, type HighlighterGeneric } from "shiki";

export type ShikiCodeLanguage =
  | "bash"
  | "c"
  | "cpp"
  | "csharp"
  | "css"
  | "diff"
  | "go"
  | "graphql"
  | "html"
  | "ini"
  | "java"
  | "javascript"
  | "json"
  | "kotlin"
  | "less"
  | "lua"
  | "makefile"
  | "markdown"
  | "objective-c"
  | "perl"
  | "php"
  | "python"
  | "r"
  | "ruby"
  | "rust"
  | "scss"
  | "sql"
  | "swift"
  | "typescript"
  | "xml"
  | "yaml";

export type ShikiTokenSpan = {
  from: number;
  to: number;
  color?: string;
  fontStyle?: number;
};

const SHIKI_THEME = "github-dark-default";
const MAX_HIGHLIGHT_CHARS = 24_000;
const MAX_CACHE_ENTRIES = 80;

const SHIKI_LANGUAGES: ShikiCodeLanguage[] = [
  "bash",
  "c",
  "cpp",
  "csharp",
  "css",
  "diff",
  "go",
  "graphql",
  "html",
  "ini",
  "java",
  "javascript",
  "json",
  "kotlin",
  "less",
  "lua",
  "makefile",
  "markdown",
  "objective-c",
  "perl",
  "php",
  "python",
  "r",
  "ruby",
  "rust",
  "scss",
  "sql",
  "swift",
  "typescript",
  "xml",
  "yaml",
];

let highlighterPromise: Promise<HighlighterGeneric<ShikiCodeLanguage, typeof SHIKI_THEME>> | null =
  null;

const tokenCache = new Map<string, ShikiTokenSpan[]>();

export function isShikiCodeLanguage(language: string): language is ShikiCodeLanguage {
  return (SHIKI_LANGUAGES as string[]).includes(language);
}

export async function highlightCodeWithShiki(
  code: string,
  language: ShikiCodeLanguage,
): Promise<ShikiTokenSpan[]> {
  if (!code || code.length > MAX_HIGHLIGHT_CHARS) return [];

  const cacheKey = `${language}\0${code}`;
  const cached = tokenCache.get(cacheKey);
  if (cached) return cached;

  const highlighter = await getShikiHighlighter();
  const lines = highlighter.codeToTokensBase(code, {
    lang: language,
    theme: SHIKI_THEME,
  });

  const spans: ShikiTokenSpan[] = [];
  for (const line of lines) {
    for (const token of line) {
      if (!token.content || !token.color) continue;
      spans.push({
        from: token.offset,
        to: token.offset + token.content.length,
        color: token.color,
        fontStyle: token.fontStyle,
      });
    }
  }

  tokenCache.set(cacheKey, spans);
  if (tokenCache.size > MAX_CACHE_ENTRIES) {
    tokenCache.delete(tokenCache.keys().next().value);
  }
  return spans;
}

function getShikiHighlighter(): Promise<HighlighterGeneric<ShikiCodeLanguage, typeof SHIKI_THEME>> {
  highlighterPromise ??= createHighlighter({
    themes: [SHIKI_THEME],
    langs: SHIKI_LANGUAGES,
  });
  return highlighterPromise;
}
