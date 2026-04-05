/**
 * E2E sync tests: local entry -> Ark event, and Ark event -> local entry.
 *
 * Verifies the full pipeline:
 *   1. Adding a meal via nutrition-store sends the correct ArkChange
 *   2. Adding water via water-store sends the correct ArkChange
 *   3. Incoming Ark events are correctly mapped back to local entries
 */

import type { ArkChange } from "../ark-client";

import type { FoodItem, MealEntry } from "@/types/nutrition";

// ---------------------------------------------------------------------------

// Mocks — must be declared before imports that use them

// ---------------------------------------------------------------------------

// Capture all sendChange calls

const sendChangeSpy = jest.fn<boolean, [ArkChange]>().mockReturnValue(true);

// Mock ark-client singleton

jest.mock("../ark-client", () => ({
  ArkSyncClient: jest.fn(),

  arkSync: {
    sendChange: (...args: unknown[]) => sendChangeSpy(...(args as [ArkChange])),

    onChange: jest.fn(() => jest.fn()),

    onStatus: jest.fn(() => jest.fn()),

    connect: jest.fn(),

    disconnect: jest.fn(),

    get isConnected() {
      return true;
    },
  },
}));

// Mock database layer — all DB ops become no-ops

jest.mock("@/db/database", () => ({
  getSetting: jest.fn((_key: string, def: string) => def),

  setSetting: jest.fn(),

  loadAllNutritionEntries: jest.fn(() => []),

  insertNutritionEntry: jest.fn(),

  updateNutritionQuantity: jest.fn(),

  deleteNutritionEntry: jest.fn(),

  loadAllWaterEntries: jest.fn(() => []),

  insertWaterEntry: jest.fn(),

  deleteWaterEntry: jest.fn(),
}));

// Mock pairing module (imported by sync-store)

jest.mock("../pairing", () => ({
  claimPairingCode: jest.fn(),
}));

// Mock date-fns format used for todayKey()

jest.mock("date-fns", () => ({
  format: jest.fn(() => "2026-03-27"),
}));

// ---------------------------------------------------------------------------

// Now import the stores (they will use mocked deps)

// ---------------------------------------------------------------------------

import { useNutritionStore } from "@/stores/nutrition-store";

import { useWaterStore } from "@/stores/water-store";

import {
  mealEntryToArkEvent,
  arkEventToMealEntry,
  waterEntryToArkEvent,
  arkEventToWaterEntry,
} from "../mapper";

import {
  insertNutritionEntry,
  updateNutritionQuantity,
  deleteNutritionEntry,
  insertWaterEntry,
  deleteWaterEntry,
} from "@/db/database";

import { registerNutritionRefresh, registerWaterRefresh } from "../sync-store";

// ---------------------------------------------------------------------------

// Fixtures

// ---------------------------------------------------------------------------

const testFood: FoodItem = {
  id: "food-greek-yogurt",

  name: "Греческий йогурт",

  brand: "Danone",

  servingSize: 150,

  servingUnit: "г",

  macros: { calories: 90, protein: 10, fat: 3, carbs: 6 },
};

// ---------------------------------------------------------------------------

// Helpers

// ---------------------------------------------------------------------------

function resetStores() {
  // Reset zustand stores to initial state

  useNutritionStore.setState({
    entriesByDate: {},

    goals: { calories: 2230, protein: 150, fat: 70, carbs: 250 },

    selectedDate: "2026-03-27",

    _hydrated: false,
  });

  useWaterStore.setState({
    entriesByDate: {},

    goal: 2500,

    _hydrated: false,
  });
}

// ---------------------------------------------------------------------------

// Tests

// ---------------------------------------------------------------------------

beforeEach(() => {
  jest.clearAllMocks();

  resetStores();

  // Hydrate so stores are ready (with mocked DB returning empty)

  useNutritionStore.getState().hydrate();

  useWaterStore.getState().hydrate();
});

// ========================== OUTGOING: Local -> Ark ==========================

describe("Outgoing sync: local entries -> Ark events", () => {
  test("adding a meal sends a nutrition.meal create event via arkSync", () => {
    const store = useNutritionStore.getState();

    store.addEntry(testFood, 2, "lunch");

    expect(sendChangeSpy).toHaveBeenCalledTimes(1);

    const sentChange: ArkChange = sendChangeSpy.mock.calls[0][0];

    expect(sentChange.change_type).toBe("create");

    expect(sentChange.event_id).toBeTruthy();

    // Verify the Ark event structure

    const data = sentChange.data as Record<string, unknown>;

    expect(data.event_type).toBe("nutrition.meal");

    expect(data.category).toBe("nutrition");

    expect(data.source).toBe("elysium");

    const inner = data.data as Record<string, unknown>;

    expect(inner.food_item).toEqual(testFood);

    expect(inner.quantity).toBe(2);

    expect(inner.meal_type).toBe("lunch");

    expect(inner.date).toBe("2026-03-27");
  });

  test("updating a meal entry sends an update event", () => {
    const store = useNutritionStore.getState();

    // First add an entry

    store.addEntry(testFood, 1, "breakfast");

    const addedEntries =
      useNutritionStore.getState().entriesByDate["2026-03-27"];

    expect(addedEntries).toHaveLength(1);

    const entryId = addedEntries[0].id;

    sendChangeSpy.mockClear();

    // Now update quantity

    useNutritionStore.getState().updateEntryQuantity(entryId, 3);

    expect(sendChangeSpy).toHaveBeenCalledTimes(1);

    const sentChange: ArkChange = sendChangeSpy.mock.calls[0][0];

    expect(sentChange.change_type).toBe("update");

    const inner = (sentChange.data as Record<string, unknown>).data as Record<
      string,
      unknown
    >;

    expect(inner.quantity).toBe(3);
  });

  test("removing a meal entry sends a delete event", () => {
    const store = useNutritionStore.getState();

    store.addEntry(testFood, 1, "dinner");

    const entryId =
      useNutritionStore.getState().entriesByDate["2026-03-27"][0].id;

    sendChangeSpy.mockClear();

    useNutritionStore.getState().removeEntry(entryId);

    expect(sendChangeSpy).toHaveBeenCalledTimes(1);

    const sentChange: ArkChange = sendChangeSpy.mock.calls[0][0];

    expect(sentChange.change_type).toBe("delete");

    expect(sentChange.event_id).toBe(entryId);
  });

  test("adding water sends a nutrition.water create event via arkSync", () => {
    const store = useWaterStore.getState();

    store.addWater(350);

    expect(sendChangeSpy).toHaveBeenCalledTimes(1);

    const sentChange: ArkChange = sendChangeSpy.mock.calls[0][0];

    expect(sentChange.change_type).toBe("create");

    const data = sentChange.data as Record<string, unknown>;

    expect(data.event_type).toBe("nutrition.water");

    expect(data.source).toBe("elysium");

    expect(data.summary).toBe("350 мл воды");

    const inner = data.data as Record<string, unknown>;

    expect(inner.amount).toBe(350);

    expect(inner.date).toBe("2026-03-27");
  });

  test("removing water sends a delete event", () => {
    const store = useWaterStore.getState();

    store.addWater(250);

    const entryId = useWaterStore.getState().entriesByDate["2026-03-27"][0].id;

    sendChangeSpy.mockClear();

    useWaterStore.getState().removeWater(entryId);

    expect(sendChangeSpy).toHaveBeenCalledTimes(1);

    expect(sendChangeSpy.mock.calls[0][0].change_type).toBe("delete");
  });
});

// ======================== INCOMING: Ark -> Local ============================

describe("Incoming sync: Ark events -> local entries", () => {
  test("incoming nutrition.meal create event is mapped to a valid MealEntry", () => {
    const incomingEvent: ArkChange = {
      event_id: "evt-meal-001",

      change_type: "create",

      data: {
        event_type: "nutrition.meal",

        category: "nutrition",

        source: "elysium",

        source_id: "entry-001",

        occurred_at: "2026-03-27T12:00:00Z",

        summary: "Греческий йогурт x2",

        tags: ["nutrition", "lunch"],

        data: {
          date: "2026-03-27",

          food_item: testFood,

          quantity: 2,

          meal_type: "lunch",

          logged_at: "2026-03-27T12:00:00Z",
        },
      },

      device_id: "other-device",

      device_seq: 5,
    };

    const result = arkEventToMealEntry(incomingEvent);

    expect(result).not.toBeNull();

    expect(result!.date).toBe("2026-03-27");

    expect(result!.entry.id).toBe("entry-001");

    expect(result!.entry.foodItem).toEqual(testFood);

    expect(result!.entry.quantity).toBe(2);

    expect(result!.entry.mealType).toBe("lunch");

    expect(result!.entry.loggedAt).toBe("2026-03-27T12:00:00Z");
  });

  test("incoming nutrition.water create event is mapped to a valid WaterEntry", () => {
    const incomingEvent: ArkChange = {
      event_id: "evt-water-001",

      change_type: "create",

      data: {
        event_type: "nutrition.water",

        category: "nutrition",

        source: "elysium",

        source_id: "water-001",

        occurred_at: "2026-03-27T14:30:00Z",

        summary: "350 мл воды",

        tags: ["nutrition", "water"],

        data: {
          date: "2026-03-27",

          amount: 350,

          time: "2026-03-27T14:30:00Z",
        },
      },

      device_id: "other-device",

      device_seq: 6,
    };

    const result = arkEventToWaterEntry(incomingEvent);

    expect(result).not.toBeNull();

    expect(result!.date).toBe("2026-03-27");

    expect(result!.entry.id).toBe("water-001");

    expect(result!.entry.amount).toBe(350);

    expect(result!.entry.time).toBe("2026-03-27T14:30:00Z");
  });

  test("incoming event with missing data returns null gracefully", () => {
    const badEvent: ArkChange = {
      event_id: "bad-evt",

      change_type: "create",

      data: { event_type: "nutrition.meal" },
    };

    expect(arkEventToMealEntry(badEvent)).toBeNull();

    expect(arkEventToWaterEntry(badEvent)).toBeNull();
  });
});

// ===================== ROUND-TRIP: Local -> Ark -> Local ====================

describe("Round-trip: entry -> Ark event -> entry", () => {
  test("meal entry survives a round-trip through Ark event format", () => {
    const original: MealEntry = {
      id: "roundtrip-meal-001",

      foodItem: testFood,

      quantity: 1.5,

      mealType: "breakfast",

      loggedAt: "2026-03-27T08:00:00Z",
    };

    const date = "2026-03-27";

    // Local -> Ark

    const arkEvent = mealEntryToArkEvent(original, date, "create");

    // Ark -> Local

    const parsed = arkEventToMealEntry(arkEvent);

    expect(parsed).not.toBeNull();

    expect(parsed!.date).toBe(date);

    expect(parsed!.entry.id).toBe(original.id);

    expect(parsed!.entry.foodItem).toEqual(original.foodItem);

    expect(parsed!.entry.quantity).toBe(original.quantity);

    expect(parsed!.entry.mealType).toBe(original.mealType);

    expect(parsed!.entry.loggedAt).toBe(original.loggedAt);
  });

  test("water entry survives a round-trip through Ark event format", () => {
    const original = {
      id: "roundtrip-water-001",
      amount: 500,
      time: "2026-03-27T15:00:00Z",
    };

    const date = "2026-03-27";

    const arkEvent = waterEntryToArkEvent(original, date, "create");

    const parsed = arkEventToWaterEntry(arkEvent);

    expect(parsed).not.toBeNull();

    expect(parsed!.date).toBe(date);

    expect(parsed!.entry.id).toBe(original.id);

    expect(parsed!.entry.amount).toBe(original.amount);

    expect(parsed!.entry.time).toBe(original.time);
  });
});

// ================== SYNC-STORE: applyIncomingChange =========================

describe("sync-store: applyIncomingChange via onChange handlers", () => {
  // We test the applyIncomingChange logic indirectly by importing the

  // sync-store module which registers handlers on arkSync.onChange.

  // Since arkSync is mocked, we capture the handler and call it manually.

  let capturedOnChangeHandler: ((change: ArkChange) => void) | null = null;

  beforeEach(() => {
    // The sync-store's connect() calls arkSync.onChange — grab the handler

    const { arkSync } = jest.requireMock("../ark-client") as {
      arkSync: { onChange: jest.Mock; onStatus: jest.Mock };
    };

    arkSync.onChange.mockImplementation(
      (handler: (change: ArkChange) => void) => {
        capturedOnChangeHandler = handler;

        return jest.fn();
      },
    );

    arkSync.onStatus.mockImplementation(
      (handler: (connected: boolean) => void) => {
        // Immediately report connected

        handler(true);

        return jest.fn();
      },
    );

    // Import and hydrate sync store, then connect

    const { useSyncStore } =
      require("../sync-store") as typeof import("../sync-store");

    useSyncStore.setState({
      serverUrl: "ws://test-server",

      apiKey: "test-key",

      deviceId: "test-device",
    });

    useSyncStore.getState().connect();
  });

  test("incoming meal create event calls insertNutritionEntry", () => {
    expect(capturedOnChangeHandler).not.toBeNull();

    const incomingMeal: ArkChange = {
      event_id: "remote-meal-001",

      change_type: "create",

      data: {
        event_type: "nutrition.meal",

        source_id: "remote-meal-001",

        occurred_at: "2026-03-27T12:00:00Z",

        data: {
          date: "2026-03-27",

          food_item: testFood,

          quantity: 1,

          meal_type: "lunch",

          logged_at: "2026-03-27T12:00:00Z",
        },
      },

      device_id: "other-device",

      device_seq: 10,
    };

    capturedOnChangeHandler!(incomingMeal);

    expect(insertNutritionEntry).toHaveBeenCalledWith(
      expect.objectContaining({
        id: "remote-meal-001",

        date: "2026-03-27",

        quantity: 1,

        meal_type: "lunch",
      }),
    );
  });

  test("incoming meal delete event calls deleteNutritionEntry", () => {
    expect(capturedOnChangeHandler).not.toBeNull();

    const deleteEvent: ArkChange = {
      event_id: "remote-meal-001",

      change_type: "delete",

      data: { event_type: "nutrition.meal" },

      device_id: "other-device",

      device_seq: 11,
    };

    capturedOnChangeHandler!(deleteEvent);

    expect(deleteNutritionEntry).toHaveBeenCalledWith("remote-meal-001");
  });

  test("incoming meal update event calls updateNutritionQuantity", () => {
    expect(capturedOnChangeHandler).not.toBeNull();

    const updateEvent: ArkChange = {
      event_id: "remote-meal-001",

      change_type: "update",

      data: {
        event_type: "nutrition.meal",

        source_id: "remote-meal-001",

        data: {
          date: "2026-03-27",

          food_item: testFood,

          quantity: 5,

          meal_type: "lunch",

          logged_at: "2026-03-27T12:00:00Z",
        },
      },

      device_id: "other-device",

      device_seq: 12,
    };

    capturedOnChangeHandler!(updateEvent);

    expect(updateNutritionQuantity).toHaveBeenCalledWith("remote-meal-001", 5);
  });

  test("incoming water create event calls insertWaterEntry", () => {
    expect(capturedOnChangeHandler).not.toBeNull();

    const incomingWater: ArkChange = {
      event_id: "remote-water-001",

      change_type: "create",

      data: {
        event_type: "nutrition.water",

        source_id: "remote-water-001",

        occurred_at: "2026-03-27T14:00:00Z",

        data: {
          date: "2026-03-27",

          amount: 400,

          time: "2026-03-27T14:00:00Z",
        },
      },

      device_id: "other-device",

      device_seq: 13,
    };

    capturedOnChangeHandler!(incomingWater);

    expect(insertWaterEntry).toHaveBeenCalledWith(
      expect.objectContaining({
        id: "remote-water-001",

        date: "2026-03-27",

        amount: 400,
      }),
    );
  });

  test("incoming water delete event calls deleteWaterEntry", () => {
    expect(capturedOnChangeHandler).not.toBeNull();

    const deleteEvent: ArkChange = {
      event_id: "remote-water-001",

      change_type: "delete",

      data: { event_type: "nutrition.water" },

      device_id: "other-device",

      device_seq: 14,
    };

    capturedOnChangeHandler!(deleteEvent);

    expect(deleteWaterEntry).toHaveBeenCalledWith("remote-water-001");
  });
});
