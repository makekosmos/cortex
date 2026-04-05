import type { FoodItem } from "@/types/nutrition";

const OFF_BASE = "https://world.openfoodfacts.org";

interface OFFNutriments {
  "energy-kcal_100g"?: number;

  "energy-kcal"?: number;

  "energy-kj_100g"?: number;

  "energy-kj"?: number;

  proteins_100g?: number;

  proteins?: number;

  fat_100g?: number;

  fat?: number;

  carbohydrates_100g?: number;

  carbohydrates?: number;
}

interface OFFProduct {
  product_name?: string;

  brands?: string;

  serving_quantity?: number;

  serving_size?: string;

  nutriments?: OFFNutriments;
}

interface OFFResponse {
  status: number;

  product?: OFFProduct;
}

interface OFFSearchResponse {
  count: number;

  products: OFFProduct[];
}

function parseFoodItem(product: OFFProduct, barcode?: string): FoodItem | null {
  const name = product.product_name?.trim();

  if (!name) return null;

  const n = product.nutriments ?? {};

  const protein = n.proteins_100g ?? n.proteins ?? 0;

  const fat = n.fat_100g ?? n.fat ?? 0;

  const carbs = n.carbohydrates_100g ?? n.carbohydrates ?? 0;

  const rawCal = n["energy-kcal_100g"] ?? n["energy-kcal"] ?? 0;

  // Many products only have kJ — convert to kcal (1 kcal ≈ 4.184 kJ)

  const kjCal =
    rawCal > 0
      ? rawCal
      : Math.round((n["energy-kj_100g"] ?? n["energy-kj"] ?? 0) / 4.184);

  const cal = kjCal > 0 ? kjCal : protein * 4 + carbs * 4 + fat * 9;

  return {
    id:
      barcode ?? `off-${Date.now()}-${Math.random().toString(36).slice(2, 8)}`,

    name,

    brand: product.brands?.split(",")[0]?.trim() || undefined,

    servingSize: 100,

    servingUnit: "г",

    macros: {
      calories: Math.round(cal),

      protein: Math.round(protein * 10) / 10,

      fat: Math.round(fat * 10) / 10,

      carbs: Math.round(carbs * 10) / 10,
    },
  };
}

export async function lookupBarcode(barcode: string): Promise<FoodItem | null> {
  try {
    const res = await fetch(
      `${OFF_BASE}/api/v2/product/${barcode}?fields=product_name,brands,nutriments,serving_quantity,serving_size`,
      {
        headers: { "User-Agent": "Elysium/1.0 (SelfSuite nutrition tracker)" },
      },
    );

    if (!res.ok) return null;

    const data: OFFResponse = await res.json();

    if (data.status !== 1 || !data.product) return null;

    return parseFoodItem(data.product, barcode);
  } catch {
    return null;
  }
}

export async function searchOpenFoodFacts(
  query: string,
  page = 1,
): Promise<FoodItem[]> {
  try {
    const q = query.toLowerCase();

    // Stem: drop last 1-2 chars to handle Russian declensions (яйцо→яйц, молоко→молок)

    const stem =
      q.length >= 4 ? q.slice(0, -2) : q.length >= 3 ? q.slice(0, -1) : q;

    // V1 CGI search with Russian language filter — V2 API ignores lc/cc params

    const params = new URLSearchParams({
      search_terms: query,

      search_simple: "1",

      action: "process",

      page: String(page),

      page_size: "50",

      json: "1",

      sort_by: "unique_scans_n",

      tagtype_0: "languages",

      tag_contains_0: "contains",

      tag_0: "ru",

      fields:
        "product_name,brands,nutriments,serving_quantity,serving_size,code",
    });

    const res = await fetch(`${OFF_BASE}/cgi/search.pl?${params}`, {
      headers: { "User-Agent": "Elysium/1.0 (SelfSuite nutrition tracker)" },
    });

    if (!res.ok) return [];

    const data: OFFSearchResponse = await res.json();

    const all = data.products

      .map((p) => parseFoodItem(p))

      .filter((f): f is FoodItem => f !== null);

    // Filter by stem to handle morphology (яйцо matches яйца, яйце, etc.)

    const matched = all.filter((f) => {
      const name = f.name.toLowerCase();

      const brand = f.brand?.toLowerCase() ?? "";

      return name.includes(stem) || brand.includes(stem);
    });

    return matched.length > 0 ? matched.slice(0, 20) : all.slice(0, 20);
  } catch {
    return [];
  }
}
