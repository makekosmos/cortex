import assert from "node:assert/strict";
import test from "node:test";

import { dateIntToIso, foodEntries } from "../helpers.mjs";
import { mapDiaryEntry, NUTRITION_ENTRY_TYPE } from "../mapping.mjs";
import { fixture } from "./support.mjs";

test("official food_entries.food_entry fixture maps to nutrition_entry_obj", () => {
  const object = mapDiaryEntry(fixture.food_entries.food_entry);
  assert.equal(object.id, "fatsecret-food-entry:entry-100");
  assert.equal(object.typeId, NUTRITION_ENTRY_TYPE);
  assert.equal(object.data.date, "2026-08-27");
  assert.equal(object.data.nutrients.calories, 320);
  assert.equal(object.data.nutrients.carbohydrate, 54.5);
  assert.equal("fiber" in object.data.nutrients, false);
});

test("all official nutrient fields map and unavailable values are omitted", () => {
  const object = mapDiaryEntry({
    food_entry_id: "official-1",
    date_int: 20692,
    food_entry_name: "Soup",
    food_entry_description: "Soup description",
    saturated_fat: "2.5",
    polyunsaturated_fat: "1",
    monounsaturated_fat: "2",
    trans_fat: "0",
    cholesterol: "4",
    sodium: "20",
    potassium: "100",
    added_sugars: "3",
    vitamin_d: "4",
    vitamin_a: "5",
    vitamin_c: "6",
    calcium: "7",
    iron: "8",
  });
  assert.equal(object.data.date, "2026-08-27");
  assert.equal(object.data.foodName, "Soup");
  assert.deepEqual(object.data.nutrients, {
    saturated_fat: 2.5,
    polyunsaturated_fat: 1,
    monounsaturated_fat: 2,
    trans_fat: 0,
    cholesterol: 4,
    sodium: 20,
    potassium: 100,
    added_sugars: 3,
    vitamin_d: 4,
    vitamin_a: 5,
    vitamin_c: 6,
    calcium: 7,
    iron: 8,
  });
});

test("date_int uses UTC epoch days and rejects invalid values", () => {
  assert.equal(dateIntToIso("20692"), "2026-08-27");
  assert.throws(() => dateIntToIso("not-a-day"), { kind: "api" });
  assert.throws(() => mapDiaryEntry({ food_entry_id: "bad", date_int: 1.5 }), { kind: "api" });
});

test("response parsing reads only official food_entries.food_entry", () => {
  assert.deepEqual(foodEntries('{"food_entries":{"food_entry":{"food_entry_id":"one"}}}'), [
    { food_entry_id: "one" },
  ]);
  assert.throws(() => foodEntries('{"diary_entries":{"food_entry":[]}}'), {
    kind: "invalid_response",
  });
});
