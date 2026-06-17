export type KosmosWindowMaterial = "acrylic" | "mica" | "none";
export type KosmosWindowEffects = "flat" | "acrylic" | "mica";

interface WindowEffectsEnv {
  KOSMOS_WINDOW_EFFECTS?: string;
  KEPLER_BG_MATERIAL?: string;
}

export function normalizeWindowEffects(value: string | undefined): KosmosWindowEffects | null {
  const v = (value ?? "").trim().toLowerCase();
  if (v === "flat" || v === "acrylic" || v === "mica") return v;
  return null;
}

export function normalizeLegacyBgMaterial(value: string | undefined): KosmosWindowMaterial | null {
  const v = (value ?? "").trim().toLowerCase();
  if (v === "none" || v === "acrylic" || v === "mica") return v;
  return null;
}

export function resolveWindowMaterial(
  fallback: KosmosWindowMaterial,
  env: WindowEffectsEnv = process.env,
): KosmosWindowMaterial {
  const globalEffects = normalizeWindowEffects(env.KOSMOS_WINDOW_EFFECTS);
  if (globalEffects === "flat") return "none";
  if (globalEffects) return globalEffects;

  const legacy = normalizeLegacyBgMaterial(env.KEPLER_BG_MATERIAL);
  return legacy ?? fallback;
}

export function backgroundMaterialOption(material: KosmosWindowMaterial): {
  backgroundMaterial: KosmosWindowMaterial;
} {
  return { backgroundMaterial: material };
}

export function applyWindowMaterial(
  win: { setBackgroundMaterial: (material: KosmosWindowMaterial) => void },
  material: KosmosWindowMaterial,
  label: string,
): void {
  try {
    win.setBackgroundMaterial(material);
  } catch (e) {
    console.error(`[kepler-shell] ${label} setBackgroundMaterial failed:`, e);
  }
}
