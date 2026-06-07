// usePlatform — единый источник платформы для renderer-кода.
//
// Источник истины — `data-platform` на `<html>`, который preload (shell +
// extension) ставит из Node `process.platform` (см.
// platform/desktop/electron/platform-chrome.ts). Фолбэк — `navigator.platform`
// для не-Electron контекстов (Storybook, браузер, тесты).
//
// Использовать вместо разрозненных `navigator.platform.startsWith("Mac")`
// проверок: один сигнал, единый маппинг, переживает спуфинг userAgent.

import { computed, type ComputedRef } from "vue";
import type { TitlebarPlatform } from "../components/types";

function detectPlatform(): TitlebarPlatform {
  // 1) data-platform от preload (надёжнее всего в Electron).
  if (typeof document !== "undefined") {
    const marker = document.documentElement.dataset.platform;
    if (marker === "mac" || marker === "windows" || marker === "linux") {
      return marker;
    }
  }
  // 2) Фолбэк на navigator (Storybook / браузер / до preload).
  if (typeof navigator !== "undefined") {
    const p = navigator.platform || "";
    if (/Mac/i.test(p)) return "mac";
    if (/Linux/i.test(p)) return "linux";
  }
  return "windows";
}

export interface PlatformInfo {
  /** Стабильный токен платформы для DesktopChrome/Titlebar `platform` prop. */
  platform: ComputedRef<TitlebarPlatform>;
  isMac: ComputedRef<boolean>;
  isWindows: ComputedRef<boolean>;
  isLinux: ComputedRef<boolean>;
}

/**
 * Возвращает реактивную платформу. Значение стабильно на время жизни окна
 * (платформа не меняется), но обёрнуто в computed для удобного шаблонного
 * использования и единообразия с остальными composable'ами.
 */
export function usePlatform(): PlatformInfo {
  const platform = computed<TitlebarPlatform>(() => detectPlatform());
  return {
    platform,
    isMac: computed(() => platform.value === "mac"),
    isWindows: computed(() => platform.value === "windows"),
    isLinux: computed(() => platform.value === "linux"),
  };
}
