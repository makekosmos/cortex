export type KosmosWindowMaterial = "acrylic" | "mica" | "none";
type KosmosWindowEffects = "flat" | "acrylic" | "mica";

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
  void fallback;
  void env;
  // ponytail: global flat mode; restore env/fallback handling only if backdrops return.
  return "none";
}

interface BackgroundMaterialOption {
  backgroundMaterial: KosmosWindowMaterial;
}

export function backgroundMaterialOption(material: KosmosWindowMaterial): BackgroundMaterialOption {
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
