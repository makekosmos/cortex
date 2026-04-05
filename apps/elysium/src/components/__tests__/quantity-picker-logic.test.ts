/**
 * Test the QuantityPicker math logic in isolation.
 * We extract the pure functions to verify multiplier, mode switching, and macro calculations.
 */

interface FoodMacros {
  calories: number;

  protein: number;

  fat: number;

  carbs: number;
}

interface Food {
  servingSize: number;

  macros: FoodMacros;
}

// ── Extract logic from QuantityPicker ──

function calcMultiplier(
  mode: "servings" | "grams",
  servings: number,
  grams: number,
  servingSize: number,
): number {
  return mode === "servings"
    ? servings
    : servingSize > 0
      ? grams / servingSize
      : 0;
}

function calcMacros(food: Food, multiplier: number) {
  return {
    calories: food.macros.calories * multiplier,

    protein: food.macros.protein * multiplier,

    fat: food.macros.fat * multiplier,

    carbs: food.macros.carbs * multiplier,
  };
}

function switchToGrams(servings: number, servingSize: number): number {
  return Math.round(servings * servingSize);
}

function switchToServings(grams: number, servingSize: number): number {
  return servingSize > 0 ? Math.round((grams / servingSize) * 10) / 10 : 1;
}

function initGrams(initialQuantity: number, servingSize: number): number {
  return Math.round(initialQuantity * servingSize);
}

// ── Tests ──

const egg: Food = {
  servingSize: 100,

  macros: { calories: 157, protein: 12.7, fat: 10.9, carbs: 0.7 },
};

const milk: Food = {
  servingSize: 250,

  macros: { calories: 130, protein: 8, fat: 6.25, carbs: 11.75 },
};

const zeroServing: Food = {
  servingSize: 0,

  macros: { calories: 100, protein: 10, fat: 5, carbs: 20 },
};

describe("QuantityPicker multiplier", () => {
  test("servings mode: 1 serving = multiplier 1", () => {
    expect(calcMultiplier("servings", 1, 100, 100)).toBe(1);
  });

  test("servings mode: 2.5 servings = multiplier 2.5", () => {
    expect(calcMultiplier("servings", 2.5, 100, 100)).toBe(2.5);
  });

  test("grams mode: 100g of 100g serving = multiplier 1", () => {
    expect(calcMultiplier("grams", 1, 100, 100)).toBe(1);
  });

  test("grams mode: 200g of 100g serving = multiplier 2", () => {
    expect(calcMultiplier("grams", 1, 200, 100)).toBe(2);
  });

  test("grams mode: 50g of 100g serving = multiplier 0.5", () => {
    expect(calcMultiplier("grams", 1, 50, 100)).toBe(0.5);
  });

  test("grams mode: 500g of 250g serving = multiplier 2", () => {
    expect(calcMultiplier("grams", 1, 500, 250)).toBe(2);
  });

  test("grams mode: 0 serving size = multiplier 0", () => {
    expect(calcMultiplier("grams", 1, 100, 0)).toBe(0);
  });
});

describe("QuantityPicker macros calculation", () => {
  test("1 serving of egg = exact macros", () => {
    const m = calcMacros(egg, 1);

    expect(m.calories).toBe(157);

    expect(m.protein).toBe(12.7);

    expect(m.fat).toBe(10.9);

    expect(m.carbs).toBe(0.7);
  });

  test("200g of egg (2x) = double macros", () => {
    const multiplier = calcMultiplier("grams", 1, 200, 100);

    const m = calcMacros(egg, multiplier);

    expect(m.calories).toBe(314);

    expect(m.protein).toBeCloseTo(25.4);

    expect(m.fat).toBeCloseTo(21.8);

    expect(m.carbs).toBeCloseTo(1.4);
  });

  test("250g of milk (1 serving) = exact macros", () => {
    const multiplier = calcMultiplier("grams", 1, 250, 250);

    const m = calcMacros(milk, multiplier);

    expect(m.calories).toBe(130);

    expect(m.protein).toBe(8);
  });

  test("125g of milk (half serving) = half macros", () => {
    const multiplier = calcMultiplier("grams", 1, 125, 250);

    const m = calcMacros(milk, multiplier);

    expect(m.calories).toBe(65);

    expect(m.protein).toBe(4);
  });
});

describe("QuantityPicker mode switching", () => {
  test("1 serving of 100g → switch to grams = 100", () => {
    expect(switchToGrams(1, 100)).toBe(100);
  });

  test("2.5 servings of 100g → switch to grams = 250", () => {
    expect(switchToGrams(2.5, 100)).toBe(250);
  });

  test("1 serving of 250g → switch to grams = 250", () => {
    expect(switchToGrams(1, 250)).toBe(250);
  });

  test("100g of 100g serving → switch to servings = 1", () => {
    expect(switchToServings(100, 100)).toBe(1);
  });

  test("250g of 100g serving → switch to servings = 2.5", () => {
    expect(switchToServings(250, 100)).toBe(2.5);
  });

  test("125g of 250g serving → switch to servings = 0.5", () => {
    expect(switchToServings(125, 250)).toBe(0.5);
  });

  test("0 serving size → switch to servings = 1 (fallback)", () => {
    expect(switchToServings(100, 0)).toBe(1);
  });

  test("round trip: servings → grams → servings preserves value", () => {
    const originalServings = 1.5;

    const g = switchToGrams(originalServings, 100); // 150

    const s = switchToServings(g, 100); // 1.5

    expect(s).toBe(originalServings);
  });

  test("round trip: grams → servings → grams preserves value", () => {
    const originalGrams = 200;

    const s = switchToServings(originalGrams, 100); // 2

    const g = switchToGrams(s, 100); // 200

    expect(g).toBe(originalGrams);
  });
});

describe("QuantityPicker initialization", () => {
  test("initialQuantity 1 with 100g serving → 100g", () => {
    expect(initGrams(1, 100)).toBe(100);
  });

  test("initialQuantity 2 with 100g serving → 200g", () => {
    expect(initGrams(2, 100)).toBe(200);
  });

  test("initialQuantity 0.5 with 250g serving → 125g", () => {
    expect(initGrams(0.5, 250)).toBe(125);
  });

  test("initialQuantity 1.5 with 100g serving → 150g", () => {
    expect(initGrams(1.5, 100)).toBe(150);
  });

  // BUG CHECK: default mode is "grams", so initial multiplier should use grams, not servings

  test("default grams mode: multiplier matches initialQuantity correctly", () => {
    const initialQuantity = 1;

    const servingSize = 100;

    const ig = initGrams(initialQuantity, servingSize); // 100

    const multiplier = calcMultiplier(
      "grams",
      initialQuantity,
      ig,
      servingSize,
    );

    expect(multiplier).toBe(1); // should be 1, not initialQuantity in servings sense
  });

  test("default grams mode: 2 servings init → 200g → multiplier 2", () => {
    const initialQuantity = 2;

    const servingSize = 100;

    const ig = initGrams(initialQuantity, servingSize); // 200

    const multiplier = calcMultiplier(
      "grams",
      initialQuantity,
      ig,
      servingSize,
    );

    expect(multiplier).toBe(2);
  });
});

describe("QuantityPicker weight unit detection", () => {
  function isWeightUnit(unit: string): boolean {
    const u = unit.toLowerCase().trim();

    return u === "г" || u === "мл" || u === "g" || u === "ml";
  }

  test('"г" is weight unit', () => expect(isWeightUnit("г")).toBe(true));

  test('"мл" is weight unit', () => expect(isWeightUnit("мл")).toBe(true));

  test('"g" is weight unit', () => expect(isWeightUnit("g")).toBe(true));

  test('"средний" is NOT weight unit', () =>
    expect(isWeightUnit("средний")).toBe(false));

  test('"штука" is NOT weight unit', () =>
    expect(isWeightUnit("штука")).toBe(false));

  test('"большой" is NOT weight unit', () =>
    expect(isWeightUnit("большой")).toBe(false));

  // Bug test: non-weight serving should default to servings mode

  test('egg "1 средний" should NOT use grams mode', () => {
    const servingUnit = "средний";

    const canUseGrams = isWeightUnit(servingUnit);

    expect(canUseGrams).toBe(false);

    // Default mode should be servings, not grams

    const defaultMode = canUseGrams ? "grams" : "servings";

    expect(defaultMode).toBe("servings");
  });

  test('rice "100 г" should use grams mode', () => {
    const servingUnit = "г";

    const canUseGrams = isWeightUnit(servingUnit);

    expect(canUseGrams).toBe(true);

    const defaultMode = canUseGrams ? "grams" : "servings";

    expect(defaultMode).toBe("grams");
  });
});

describe("QuantityPicker rating", () => {
  function rateFood(food: Food) {
    const m = food.macros;

    const reasons: string[] = [];

    let badCount = 0;

    const calPer100 =
      food.servingSize > 0 ? (m.calories / food.servingSize) * 100 : m.calories;

    if (calPer100 > 400) {
      reasons.push("high cal");
      badCount++;
    } else if (calPer100 > 250) {
      reasons.push("medium cal");
    }

    const fatPer100 =
      food.servingSize > 0 ? (m.fat / food.servingSize) * 100 : m.fat;

    const carbPer100 =
      food.servingSize > 0 ? (m.carbs / food.servingSize) * 100 : m.carbs;

    if (fatPer100 > 15 && carbPer100 > 30) {
      reasons.push("fat+carb combo");
      badCount++;
    }

    if (badCount >= 2) return "bad";

    if (badCount === 1) return "ok";

    return "good";
  }

  test("egg (157 kcal/100g) = good", () => {
    expect(rateFood(egg)).toBe("good");
  });

  test("high calorie food (500 kcal/100g) = ok", () => {
    expect(
      rateFood({
        servingSize: 100,
        macros: { calories: 500, protein: 5, fat: 10, carbs: 10 },
      }),
    ).toBe("ok");
  });

  test("high cal + high fat+carb = bad", () => {
    expect(
      rateFood({
        servingSize: 100,
        macros: { calories: 500, protein: 5, fat: 20, carbs: 50 },
      }),
    ).toBe("bad");
  });

  test("low calorie food = good", () => {
    expect(
      rateFood({
        servingSize: 100,
        macros: { calories: 50, protein: 5, fat: 1, carbs: 5 },
      }),
    ).toBe("good");
  });
});
