export const typography = {
  fontFamily: {
    // На Apple: -apple-system → BlinkMacSystemFont → SF Pro (системный, не требует загрузки).
    // На Windows / Linux: IBM Plex Sans → системные fallback'и.
    sans: "-apple-system, BlinkMacSystemFont, 'SF Pro Display', 'SF Pro Text', 'IBM Plex Sans', 'Segoe UI', Inter, Avenir, Helvetica, Arial, sans-serif",

    // Mono: IBM Plex Mono приоритетный (доступен на любой ОС после установки),
    // затем системные mono-стеки.
    mono: "'IBM Plex Mono', ui-monospace, SFMono-Regular, Menlo, Consolas, monospace",
  },

  fontSize: {
    xs: "0.75rem",

    sm: "0.875rem",

    base: "1rem",

    lg: "1.125rem",

    xl: "1.25rem",

    "2xl": "1.5rem",

    settingsAdvancedTitle: "24px",

    settingsAdvancedDescription: "12px",
  },

  fontWeight: {
    normal: "400",

    medium: "500",

    semibold: "600",

    bold: "700",
  },
} as const;
