type ColorBucket = { count: number; red: number; green: number; blue: number };

export function dominantImageColor(pixels: Uint8Array, channels: number): string | null {
  const colors = new Map<number, ColorBucket>();
  for (let index = 0; index < pixels.length; index += channels) {
    if (channels >= 4 && pixels[index + 3]! < 128) continue;
    const red = pixels[index]!;
    const green = pixels[index + 1]!;
    const blue = pixels[index + 2]!;
    const key = ((red >> 5) << 6) | ((green >> 5) << 3) | (blue >> 5);
    const color = colors.get(key);
    if (color) {
      color.count += 1;
      color.red += red;
      color.green += green;
      color.blue += blue;
    } else {
      colors.set(key, { count: 1, red, green, blue });
    }
  }

  let dominant: ColorBucket | undefined;
  let fallback: ColorBucket | undefined;
  for (const color of colors.values()) {
    if (!fallback || color.count > fallback.count) fallback = color;
    const red = color.red / color.count;
    const green = color.green / color.count;
    const blue = color.blue / color.count;
    if (Math.max(red, green, blue) - Math.min(red, green, blue) < 24) continue;
    if (!dominant || color.count > dominant.count) dominant = color;
  }
  dominant ??= fallback;
  return dominant
    ? `rgb(${Math.round(dominant.red / dominant.count)} ${Math.round(dominant.green / dominant.count)} ${Math.round(dominant.blue / dominant.count)})`
    : null;
}
