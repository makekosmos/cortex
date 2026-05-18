// Storybook manager UI (sidebar / header / navigation) — кастомная тёмная
// тема под Kepler design tokens. Цвета oklch значения из css-variables.css.

import { addons } from "storybook/manager-api";
import { create } from "storybook/theming";

const keplerDarkTheme = create({
  base: "dark",

  // Branding
  brandTitle: "Kepler Visuals",
  brandUrl: "https://github.com/ksanrse/kepler",
  // Скрываем брендинг logo image — нет своей иконки в visuals.
  brandImage: undefined,

  // Color palette — соответствует Kepler dark theme tokens.
  // Не используем CSS vars напрямую (manager UI не имеет к ним доступ),
  // конвертируем oklch → hex/rgb.
  colorPrimary: "#3b82f6", // ~ oklch(0.546 0.229 264.1) accent → blue
  colorSecondary: "#3b82f6",

  // App background — main panel / preview area.
  appBg: "#252525", // ~ oklch(0.18 0 0)  --color-shape-highlight-light-solid
  appContentBg: "#1a1a1a", // ~ oklch(0.145 0 0) --background
  appPreviewBg: "#1a1a1a",
  appBorderColor: "#2d2d2d",
  appBorderRadius: 8,

  // Text colors.
  textColor: "#ededed", // foreground
  textInverseColor: "#1a1a1a",
  textMutedColor: "#9ca3af", // muted-foreground

  // Toolbar — top bar над preview.
  barTextColor: "#9ca3af",
  barHoverColor: "#3b82f6",
  barSelectedColor: "#ededed",
  barBg: "#1a1a1a",

  // Form controls.
  inputBg: "#252525",
  inputBorder: "#2d2d2d",
  inputTextColor: "#ededed",
  inputBorderRadius: 6,

  // Fonts — переиспользуем Inter Variable из preview.ts CSS import.
  fontBase: '"Inter Variable", -apple-system, BlinkMacSystemFont, "Segoe UI", sans-serif',
  fontCode: '"IBM Plex Mono", ui-monospace, Consolas, monospace',
});

addons.setConfig({
  theme: keplerDarkTheme,
  // Sidebar — компактный display.
  sidebar: {
    showRoots: true,
  },
});
