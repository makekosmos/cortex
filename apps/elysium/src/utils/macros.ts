import type { Macros, MealEntry } from '@/types/nutrition';

export function sumMacros(entries: MealEntry[]): Macros {
  const totals = entries.reduce(
    (acc, entry) => ({
      calories: acc.calories + entry.foodItem.macros.calories * entry.quantity,
      protein: acc.protein + entry.foodItem.macros.protein * entry.quantity,
      fat: acc.fat + entry.foodItem.macros.fat * entry.quantity,
      carbs: acc.carbs + entry.foodItem.macros.carbs * entry.quantity,
    }),
    { calories: 0, protein: 0, fat: 0, carbs: 0 },
  );

  // Если калории не были посчитаны, вычисляем из БЖУ
  if (totals.calories === 0 && (totals.protein > 0 || totals.fat > 0 || totals.carbs > 0)) {
    totals.calories = totals.protein * 4 + totals.carbs * 4 + totals.fat * 9;
  }

  return totals;
}

export function caloriesFromMacros(protein: number, fat: number, carbs: number): number {
  return Math.round(protein * 4 + carbs * 4 + fat * 9);
}

export function formatMacro(value: number, unit = 'г'): string {
  return `${Math.round(value * 10) / 10}${unit}`;
}

export function formatCalories(value: number): string {
  return `${Math.round(value)}`;
}
