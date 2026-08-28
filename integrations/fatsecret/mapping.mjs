export const NUTRITION_ENTRY_TYPE = "nutrition_entry_obj";

const nutrientFields = {
  calories: ["calories", "kcal"],
  carbohydrate: ["carbohydrate", "carbs"],
  protein: ["protein"],
  fat: ["fat"],
  fiber: ["fiber"],
  sugar: ["sugar"],
  sodium: ["sodium"],
};

function numberOrAbsent(value) {
  if (value === undefined || value === null || value === "") return undefined;
  const number = Number(value);
  return Number.isFinite(number) ? number : undefined;
}

function firstValue(entry, names) {
  for (const name of names) {
    if (entry[name] !== undefined && entry[name] !== null && entry[name] !== "") return entry[name];
  }
  return undefined;
}

export function mapDiaryEntry(entry) {
  const externalId = String(firstValue(entry, ["food_entry_id", "entry_id", "id"]) ?? "");
  if (!externalId) throw new Error("FatSecret diary entry is missing an external ID");
  const nutrients = {};
  for (const [name, aliases] of Object.entries(nutrientFields)) {
    const value = numberOrAbsent(firstValue(entry, aliases));
    if (value !== undefined) nutrients[name] = value;
  }
  const quantity = numberOrAbsent(firstValue(entry, ["number_of_units", "quantity", "servings"]));
  const date = String(firstValue(entry, ["date", "date_int"]) ?? "");
  if (!date) throw new Error(`FatSecret entry ${externalId} is missing a date`);
  return {
    id: `fatsecret-food-entry:${externalId}`,
    typeId: NUTRITION_ENTRY_TYPE,
    data: {
      provider: "fatsecret",
      externalId,
      date,
      meal: firstValue(entry, ["meal", "meal_name"]) ?? null,
      foodId: firstValue(entry, ["food_id", "foodId"]) ?? null,
      servingId: firstValue(entry, ["serving_id", "servingId"]) ?? null,
      foodName: firstValue(entry, ["food_name", "name"]) ?? null,
      servingDescription: firstValue(entry, ["serving_description", "serving"]) ?? null,
      quantity,
      nutrients,
    },
  };
}
