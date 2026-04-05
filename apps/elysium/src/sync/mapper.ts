/**
 * Maps between Elysium domain objects and Ark event format.
 *
 * Ark events follow the schema defined in the Ark core:
 *   event_type, data (json), category, occurred_at, summary, source, source_id, tags
 */

import type { MealEntry, MealType, FoodItem } from "@/types/nutrition";

import type { ArkChange } from "./ark-client";

// ---------------------------------------------------------------------------

// Meal entries

// ---------------------------------------------------------------------------

export function mealEntryToArkEvent(
  entry: MealEntry,

  date: string,

  changeType: "create" | "update" | "delete" = "create",
): ArkChange {
  return {
    event_id: entry.id,

    change_type: changeType,

    data: {
      event_type: "nutrition.meal",

      category: "nutrition",

      source: "elysium",

      source_id: entry.id,

      occurred_at: entry.loggedAt,

      summary: `${entry.foodItem.name} x${entry.quantity}`,

      tags: ["nutrition", entry.mealType],

      data: {
        date,

        food_item: entry.foodItem,

        quantity: entry.quantity,

        meal_type: entry.mealType,

        logged_at: entry.loggedAt,
      },
    },
  };
}

export function arkEventToMealEntry(
  event: ArkChange,
): { entry: MealEntry; date: string } | null {
  const d = event.data as Record<string, unknown>;

  const inner = d.data as Record<string, unknown> | undefined;

  if (!inner) return null;

  const foodItem = inner.food_item as FoodItem | undefined;

  if (!foodItem) return null;

  return {
    entry: {
      id: (d.source_id as string) || event.event_id,

      foodItem,

      quantity: (inner.quantity as number) ?? 1,

      mealType: (inner.meal_type as MealType) ?? "snack",

      loggedAt:
        (inner.logged_at as string) ??
        (d.occurred_at as string) ??
        new Date().toISOString(),
    },

    date: (inner.date as string) ?? "",
  };
}

// ---------------------------------------------------------------------------

// Water entries

// ---------------------------------------------------------------------------

export interface WaterEntryLike {
  id: string;

  amount: number;

  time: string;
}

export function waterEntryToArkEvent(
  entry: WaterEntryLike,

  date: string,

  changeType: "create" | "update" | "delete" = "create",
): ArkChange {
  return {
    event_id: entry.id,

    change_type: changeType,

    data: {
      event_type: "nutrition.water",

      category: "nutrition",

      source: "elysium",

      source_id: entry.id,

      occurred_at: entry.time,

      summary: `${entry.amount} мл воды`,

      tags: ["nutrition", "water"],

      data: {
        date,

        amount: entry.amount,

        time: entry.time,
      },
    },
  };
}

export function arkEventToWaterEntry(
  event: ArkChange,
): { entry: WaterEntryLike; date: string } | null {
  const d = event.data as Record<string, unknown>;

  const inner = d.data as Record<string, unknown> | undefined;

  if (!inner) return null;

  return {
    entry: {
      id: (d.source_id as string) || event.event_id,

      amount: (inner.amount as number) ?? 0,

      time:
        (inner.time as string) ??
        (d.occurred_at as string) ??
        new Date().toISOString(),
    },

    date: (inner.date as string) ?? "",
  };
}
