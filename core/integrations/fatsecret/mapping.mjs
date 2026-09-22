import { dateIntToIso, FatSecretError, normalizeDate } from "./helpers.mjs";

export const NUTRITION_ENTRY_TYPE = "nutrition_entry_obj";

const nutrientFields = {
  calories: ["calories", "kcal"],
  carbohydrate: ["carbohydrate", "carbs"],
  protein: ["protein"],
  fat: ["fat"],
  saturated_fat: ["saturated_fat"],
  polyunsaturated_fat: ["polyunsaturated_fat"],
  monounsaturated_fat: ["monounsaturated_fat"],
  trans_fat: ["trans_fat"],
  cholesterol: ["cholesterol"],
  sodium: ["sodium"],
  potassium: ["potassium"],
  fiber: ["fiber"],
  sugar: ["sugar"],
  added_sugars: ["added_sugars"],
  vitamin_d: ["vitamin_d"],
  vitamin_a: ["vitamin_a"],
  vitamin_c: ["vitamin_c"],
  calcium: ["calcium"],
  iron: ["iron"],
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
  if (!externalId)
    throw new FatSecretError("api", "FatSecret diary entry is missing an external ID");
  const nutrients = {};
  for (const [name, aliases] of Object.entries(nutrientFields)) {
    const value = numberOrAbsent(firstValue(entry, [...aliases, `food_entry_${name}`]));
    if (value !== undefined) nutrients[name] = value;
  }
  const quantity = numberOrAbsent(firstValue(entry, ["number_of_units", "quantity", "servings"]));
  const dateInt = firstValue(entry, ["date_int", "food_entry_date_int"]);
  const date =
    dateInt !== undefined ? dateIntToIso(dateInt) : normalizeDate(firstValue(entry, ["date"]));
  return {
    id: `fatsecret-food-entry:${externalId}`,
    typeId: NUTRITION_ENTRY_TYPE,
    data: {
      provider: "fatsecret",
      externalId,
      date,
      meal: firstValue(entry, ["meal", "meal_name", "food_entry_meal"]) ?? null,
      foodId: firstValue(entry, ["food_id", "foodId", "food_entry_food_id"]) ?? null,
      servingId: firstValue(entry, ["serving_id", "servingId", "food_entry_serving_id"]) ?? null,
      foodName: firstValue(entry, ["food_entry_name", "food_name", "name"]) ?? null,
      servingDescription: firstValue(entry, ["serving_description", "serving"]) ?? null,
      quantity,
      nutrients,
    },
  };
}
