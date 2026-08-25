// Резолвит тип в RGBA цвет (0..1 floats) для WebGL, переиспользуя палитру
// dashboard-типов. Кеширует результаты и резолвит CSS variables/color-mix.

import { typeVisualFor } from "../dashboard/typeVisuals";

const cssColorCache = new Map<string, [number, number, number, number]>();

const FALLBACK_RGBA: [number, number, number, number] = [0.7, 0.7, 0.7, 1];

/**
 * Converts an arbitrary CSS color expression (including CSS variables,
 * color-mix() и oklch()) to an RGBA tuple with each channel in the [0, 1] range.
 *
 * Резолвим var()/color-mix через computed style на probe-элементе внутри
 * themed (.dark) дерева, затем растеризуем один пиксель и читаем getImageData —
 * это даёт конкретный sRGB цвет независимо от того, в каком цветовом
 * пространстве сериализован computed value (rgb / oklch / color(srgb …)).
 * Раньше читали `ctx.fillStyle` строкой и парсили только #hex/rgb(), из-за чего
 * oklch-токены давали [0,0,0] (чёрный). Результат кешируется по cssExpr.
 */
export function cssColorToRgba(cssExpr: string): [number, number, number, number] {
  if (!("document" in globalThis)) {
    return FALLBACK_RGBA;
  }

  const cached = cssColorCache.get(cssExpr);
  if (cached) return cached;

  try {
    // Резолвим CSS variables + color-mix через computed style.
    const probe = document.createElement("span");
    probe.style.color = cssExpr;
    document.body.appendChild(probe);
    const computed = getComputedStyle(probe).color;
    probe.remove();

    const canvas = document.createElement("canvas");
    canvas.width = 1;
    canvas.height = 1;
    const ctx = canvas.getContext("2d", { willReadFrequently: true });
    if (!ctx) return FALLBACK_RGBA;

    // Растеризуем пиксель: что бы ни вернул computed, getImageData отдаёт
    // конкретный sRGB RGBA (0..255). Если цвет не распарсился — пиксель
    // останется прозрачным (a === 0), тогда используем fallback.
    ctx.clearRect(0, 0, 1, 1);
    ctx.fillStyle = computed || cssExpr;
    ctx.fillRect(0, 0, 1, 1);
    const [r, g, b, alpha] = ctx.getImageData(0, 0, 1, 1).data;

    if (alpha === 0) return FALLBACK_RGBA;

    const result: [number, number, number, number] = [r / 255, g / 255, b / 255, alpha / 255];
    cssColorCache.set(cssExpr, result);
    return result;
  } catch {
    return FALLBACK_RGBA;
  }
}

export function colorForType(typeId: string): [number, number, number, number] {
  return cssColorToRgba(typeVisualFor(typeId).from);
}
