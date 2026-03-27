export interface Macros {
  calories: number;
  protein: number;
  fat: number;
  carbs: number;
}

export interface NutrientDetails {
  saturatedFat?: number;
  transFat?: number;
  monoFat?: number;
  polyFat?: number;
  sugar?: number;
  fiber?: number;
  sodium?: number;
  cholesterol?: number;
  potassium?: number;
}

export interface FoodItem {
  id: string;
  name: string;
  brand?: string;
  servingSize: number;
  servingUnit: string;
  macros: Macros;
  nutrients?: NutrientDetails;
}

export interface MealEntry {
  id: string;
  foodItem: FoodItem;
  quantity: number;
  mealType: MealType;
  loggedAt: string;
}

export type MealType = 'breakfast' | 'lunch' | 'dinner' | 'snack';

export const MEAL_TYPE_LABELS: Record<MealType, string> = {
  breakfast: 'Завтрак',
  lunch: 'Обед',
  dinner: 'Ужин',
  snack: 'Перекус',
};

// calories = protein*4 + carbs*4 + fat*9
export const DEFAULT_GOALS: Macros = {
  calories: 150 * 4 + 250 * 4 + 70 * 9, // 2230
  protein: 150,
  fat: 70,
  carbs: 250,
};
