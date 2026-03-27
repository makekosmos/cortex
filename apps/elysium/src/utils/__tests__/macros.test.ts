import { sumMacros, formatMacro, formatCalories, caloriesFromMacros } from '../macros';
import type { MealEntry } from '@/types/nutrition';

function makeEntry(
  macros: { calories: number; protein: number; fat: number; carbs: number },
  quantity = 1,
): MealEntry {
  return {
    id: 'test',
    foodItem: {
      id: 'food-1',
      name: 'Test Food',
      servingSize: 100,
      servingUnit: 'г',
      macros,
    },
    quantity,
    mealType: 'lunch',
    loggedAt: new Date().toISOString(),
  };
}

describe('sumMacros', () => {
  it('returns zeroes for empty array', () => {
    expect(sumMacros([])).toEqual({ calories: 0, protein: 0, fat: 0, carbs: 0 });
  });

  it('sums a single entry with quantity 1', () => {
    const entry = makeEntry({ calories: 200, protein: 20, fat: 10, carbs: 30 });
    expect(sumMacros([entry])).toEqual({ calories: 200, protein: 20, fat: 10, carbs: 30 });
  });

  it('multiplies macros by quantity', () => {
    const entry = makeEntry({ calories: 100, protein: 10, fat: 5, carbs: 15 }, 2.5);
    const result = sumMacros([entry]);
    expect(result.calories).toBe(250);
    expect(result.protein).toBe(25);
    expect(result.fat).toBe(12.5);
    expect(result.carbs).toBe(37.5);
  });

  it('sums multiple entries', () => {
    const entries = [
      makeEntry({ calories: 100, protein: 10, fat: 5, carbs: 15 }),
      makeEntry({ calories: 200, protein: 20, fat: 10, carbs: 25 }),
    ];
    const result = sumMacros(entries);
    expect(result.calories).toBe(300);
    expect(result.protein).toBe(30);
    expect(result.fat).toBe(15);
    expect(result.carbs).toBe(40);
  });

  it('calculates calories from macros when calories are zero but macros exist', () => {
    const entry = makeEntry({ calories: 0, protein: 10, fat: 5, carbs: 20 });
    const result = sumMacros([entry]);
    // protein*4 + carbs*4 + fat*9 = 40 + 80 + 45 = 165
    expect(result.calories).toBe(165);
  });

  it('keeps zero calories when all macros are zero', () => {
    const entry = makeEntry({ calories: 0, protein: 0, fat: 0, carbs: 0 });
    expect(sumMacros([entry]).calories).toBe(0);
  });
});

describe('caloriesFromMacros', () => {
  it('calculates protein*4 + carbs*4 + fat*9', () => {
    expect(caloriesFromMacros(10, 5, 20)).toBe(165);
  });

  it('returns 0 for all zeroes', () => {
    expect(caloriesFromMacros(0, 0, 0)).toBe(0);
  });

  it('rounds to nearest integer', () => {
    // 1.1*4 + 2.2*4 + 0.3*9 = 4.4 + 8.8 + 2.7 = 15.9 → 16
    expect(caloriesFromMacros(1.1, 0.3, 2.2)).toBe(16);
  });
});

describe('formatMacro', () => {
  it('formats integer values without unnecessary decimals', () => {
    expect(formatMacro(12, 'г')).toBe('12г');
  });

  it('keeps one decimal place', () => {
    expect(formatMacro(12.56, 'г')).toBe('12.6г');
  });

  it('formats with default unit', () => {
    expect(formatMacro(5.3)).toBe('5.3г');
  });

  it('handles zero', () => {
    expect(formatMacro(0)).toBe('0г');
  });

  it('formats with custom unit', () => {
    expect(formatMacro(2.5, 'мл')).toBe('2.5мл');
  });
});

describe('formatCalories', () => {
  it('rounds to integer string', () => {
    expect(formatCalories(123.7)).toBe('124');
  });

  it('handles exact integers', () => {
    expect(formatCalories(200)).toBe('200');
  });

  it('handles zero', () => {
    expect(formatCalories(0)).toBe('0');
  });
});
