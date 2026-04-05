import type { FoodItem, NutrientDetails } from "@/types/nutrition";

const SEARCH_PATH =
  "/%D0%BA%D0%B0%D0%BB%D0%BE%D1%80%D0%B8%D0%B8-%D0%BF%D0%B8%D1%82%D0%B0%D0%BD%D0%B8%D0%B5/search";

const DESKTOP_UA =
  "Mozilla/5.0 (Macintosh; Intel Mac OS X 10_15_7) AppleWebKit/537.36 (KHTML, like Gecko) Chrome/120.0.0.0 Safari/537.36";

const HEADERS = {
  "User-Agent": DESKTOP_UA,

  Accept: "text/html,application/xhtml+xml",

  "Accept-Language": "ru-RU,ru;q=0.9",
};

function parseNum(s: string): number {
  return parseFloat(s.replace(",", ".")) || 0;
}

/**
 * Parse desktop results: <a class="prominent"> + <div class="smallText">
 */

function parseDesktopResults(html: string): FoodItem[] {
  const results: FoodItem[] = [];

  const rowRe =
    /<a\s+class="prominent"\s+href="([^"]+)"[^>]*>([^<]+)<\/a>(?:\s*(?:&nbsp;)*\s*<a\s+class="brand"[^>]*>\(([^)]+)\)<\/a>)?[\s\S]*?<div\s+class="smallText[^"]*">\s*([\s\S]*?)<\/div>/g;

  let match: RegExpExecArray | null;

  while ((match = rowRe.exec(html)) !== null) {
    const item = parseNutritionLine(match[1], match[2], match[3], match[4]);

    if (item) results.push(item);
  }

  return results;
}

/**
 * Parse mobile results: <a class="inner-link"> + <div class="nowrap small-text">
 * Brand may appear as <a class="brand">(BrandName)</a> after the name link
 */

function parseMobileResults(html: string): FoodItem[] {
  const results: FoodItem[] = [];

  const rowRe =
    /<a\s+class="inner-link"\s+href="([^"]+)"[^>]*>([^<]+)<\/a>(?:\s*(?:&nbsp;)*\s*<a\s+class="brand"[^>]*>\(([^)]+)\)<\/a>)?\s*<div\s+class="nowrap\s+small-text">\s*([\s\S]*?)<\/div>/g;

  let match: RegExpExecArray | null;

  while ((match = rowRe.exec(html)) !== null) {
    const item = parseNutritionLine(match[1], match[2], match[3], match[4]);

    if (item) results.push(item);
  }

  return results;
}

function parseNutritionLine(
  href: string,
  rawName: string,
  brand: string | undefined,
  infoText: string,
): FoodItem | null {
  const name = decodeHTMLEntities(rawName.trim());

  const calMatch = infoText.match(/Калории:\s*([\d.,]+)/);

  const fatMatch = infoText.match(/Жир:\s*([\d.,]+)/);

  const carbMatch = infoText.match(/Углев:\s*([\d.,]+)/);

  const protMatch = infoText.match(/Белк:\s*([\d.,]+)/);

  const servingMatch = infoText.match(/в\s+([\d.,]+)\s*(\S+)/);

  const servingSize = servingMatch ? parseNum(servingMatch[1]) : 100;

  const servingUnit = servingMatch
    ? servingMatch[2].replace(/\s*-\s*$/, "")
    : "г";

  if (!calMatch) return null;

  return {
    id: `fs-${encodeURIComponent(href)}`,

    name,

    brand: brand?.trim() || undefined,

    servingSize,

    servingUnit,

    macros: {
      calories: Math.round(parseNum(calMatch[1])),

      protein: Math.round(parseNum(protMatch?.[1] ?? "0") * 10) / 10,

      fat: Math.round(parseNum(fatMatch?.[1] ?? "0") * 10) / 10,

      carbs: Math.round(parseNum(carbMatch?.[1] ?? "0") * 10) / 10,
    },
  };
}

function decodeHTMLEntities(s: string): string {
  return s

    .replace(/&#160;/g, " ")

    .replace(/&amp;/g, "&")

    .replace(/&lt;/g, "<")

    .replace(/&gt;/g, ">")

    .replace(/&quot;/g, '"');
}

/**
 * Parse extended nutrients from a FatSecret product detail page.
 * Format: <div class="nutrient...left">Label</div><div class="nutrient...right">Value</div>
 */

function parseDetailNutrients(html: string): NutrientDetails {
  const nutrients: NutrientDetails = {};

  const rows = html.matchAll(
    /<div\s+class="nutrient[^"]*left">([^<]+)<\/div>\s*<div\s+class="nutrient[^"]*right[^"]*">([^<]+)<\/div>/g,
  );

  for (const [, label, value] of rows) {
    const l = label.trim();

    const v = parseNum(value.replace(/[мгкДж\s]/g, ""));

    if (l.startsWith("Насыщенные")) nutrients.saturatedFat = v;
    else if (l.startsWith("Транс")) nutrients.transFat = v;
    else if (l.startsWith("Мононенасыщенные")) nutrients.monoFat = v;
    else if (l.startsWith("Полиненасыщенные")) nutrients.polyFat = v;
    else if (l === "Сахар") nutrients.sugar = v;
    else if (l === "Клетчатка") nutrients.fiber = v;
    else if (l === "Натрий") nutrients.sodium = v;
    else if (l === "Холестерин") nutrients.cholesterol = v;
    else if (l === "Калий") nutrients.potassium = v;
  }

  return nutrients;
}

/** Fetch detail page for a FatSecret food and return extended nutrients. */

export async function fetchFatSecretDetails(food: FoodItem): Promise<FoodItem> {
  // Extract href from id: "fs-/калории-питание/..."

  const href = decodeURIComponent(food.id.replace(/^fs-/, ""));

  if (!href.startsWith("/")) return food;

  try {
    for (const base of [
      "https://www.fatsecret.ru",
      "http://www.fatsecret.ru",
    ]) {
      try {
        const res = await fetch(`${base}${href}`, {
          headers: HEADERS,
          redirect: "follow",
        });

        if (!res.ok) continue;

        const html = await res.text();

        const nutrients = parseDetailNutrients(html);

        // Check if we got any data

        if (Object.values(nutrients).some((v) => v !== undefined && v > 0)) {
          return { ...food, nutrients };
        }
      } catch {
        continue;
      }
    }
  } catch {
    // ignore
  }

  return food;
}

export async function searchFatSecret(
  query: string,
  page = 1,
): Promise<FoodItem[]> {
  try {
    const params = new URLSearchParams({ q: query, pg: String(page - 1) });

    // Try HTTPS first (desktop UA to avoid mobile redirect), fall back to HTTP

    for (const base of [
      "https://www.fatsecret.ru",
      "http://www.fatsecret.ru",
    ]) {
      try {
        const url = `${base}${SEARCH_PATH}?${params}`;

        const res = await fetch(url, { headers: HEADERS, redirect: "follow" });

        if (!res.ok) continue;

        const html = await res.text();

        // Try desktop parser first, then mobile parser

        let results = parseDesktopResults(html);

        if (results.length === 0) {
          results = parseMobileResults(html);
        }

        if (results.length > 0) return results;
      } catch {
        continue;
      }
    }

    return [];
  } catch {
    return [];
  }
}
