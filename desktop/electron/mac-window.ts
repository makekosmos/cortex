// macOS-нативные опции BrowserWindow.
//
// Применяются только к chrome-окнам (settings / dashboard / extensions) с
// `titleBarStyle: "hidden"` — там macOS рисует traffic lights, которые нужно
// вертикально отцентрировать в нашем ~36px titlebar и подложить нативный
// vibrancy вместо Windows-only `backgroundMaterial: "acrylic"`.
//
// На не-macOS возвращает {} — вызывающий код просто спредит результат.

import type { BrowserWindowConstructorOptions } from "electron";

export interface MacWindowChromeOptions {
  /** Y-координата центра traffic lights относительно верха окна. Подбирается
   * под высоту titlebar'а конкретного окна (центр ≈ titlebarHeight/2 - 6). */
  trafficLightY?: number;
}

/**
 * Возвращает darwin-специфичные BrowserWindow опции (vibrancy + позиция
 * traffic lights). На Windows/Linux — пустой объект.
 */
export function macWindowChrome(
  opts: MacWindowChromeOptions = {},
): BrowserWindowConstructorOptions {
  if (process.platform !== "darwin") return {};
  return {
    // Нативный блюр-фон macOS. Виден там, где renderer оставляет фон
    // полупрозрачным; под solid-поверхностями просто не проявляется (не вредит).
    vibrancy: "under-window",
    visualEffectState: "active",
    // Центрируем traffic lights в нашем кастомном titlebar'е (по умолчанию
    // они прижаты к верху и не совпадают с нашим header'ом).
    trafficLightPosition: { x: 16, y: opts.trafficLightY ?? 12 },
  };
}
