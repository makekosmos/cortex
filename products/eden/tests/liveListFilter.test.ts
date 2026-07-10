// Тесты для логики фильтрации объектов при live-обновлении списка Eden.
// SDD → TDD: RED → GREEN.
//
// Проверяем чистую функцию shouldIncludeTypeInEdenListForLiveUpdate из
// lib/kepler-api-shim.ts — синхронную, не зависящую от ARK bridge.
//
// Логика полностью повторяет listEntries():
// 1. Если visibleTypeIds пустой → принимаем ВСЕ типы, кроме
//    невидимых collection_obj (shouldIncludeObjectInEdenList).
// 2. Если visibleTypeIds непустой → принимаем только типы из него,
//    плюс collection_obj — но только если его object_type_id виден.
// 3. collection_obj без видимого object_type_id → НЕ принимаем.
//
// Примечание: при пустом visibleTypeIds task_obj/game_obj технически МОГУТ
// попасть в listEntries (listObjectSummariesForVisibleTypes([]) → "list_object_summaries"
// → все объекты, shouldIncludeObjectInEdenList → true для не-collection типов).
// Функция реплицирует это поведение один-в-один.

import { describe, expect, test } from "bun:test";
import { shouldIncludeTypeInEdenListForLiveUpdate } from "../src/store/liveListFilter";

describe("shouldIncludeTypeInEdenListForLiveUpdate", () => {
  test("excludes bubble-marked journal objects from the ordinary Eden list", () => {
    expect(
      shouldIncludeTypeInEdenListForLiveUpdate({
        typeId: "system-type-journal",
        propsJson: { entry_kind: "bubble" },
        visibleTypeIds: [],
      }),
    ).toBe(false);
    expect(
      shouldIncludeTypeInEdenListForLiveUpdate({
        typeId: "system-type-journal",
        propsJson: {},
        visibleTypeIds: [],
      }),
    ).toBe(true);
  });
  describe("visibleTypeIds = [] (дефолт — показываем все объекты кроме скрытых коллекций)", () => {
    test("note_obj — принимаем", () => {
      expect(
        shouldIncludeTypeInEdenListForLiveUpdate({
          typeId: "note_obj",
          propsJson: {},
          visibleTypeIds: [],
        }),
      ).toBe(true);
    });

    test("person_obj — принимаем", () => {
      expect(
        shouldIncludeTypeInEdenListForLiveUpdate({
          typeId: "person_obj",
          propsJson: {},
          visibleTypeIds: [],
        }),
      ).toBe(true);
    });

    test("collection_obj с object_type_id=note_obj — принимаем (видимая коллекция)", () => {
      expect(
        shouldIncludeTypeInEdenListForLiveUpdate({
          typeId: "collection_obj",
          propsJson: { object_type_id: "note_obj" },
          visibleTypeIds: [],
        }),
      ).toBe(true);
    });

    test("collection_obj с object_type_id=task_obj — НЕ принимаем (HIDDEN_EDEN_COLLECTION_TYPE_IDS)", () => {
      expect(
        shouldIncludeTypeInEdenListForLiveUpdate({
          typeId: "collection_obj",
          propsJson: { object_type_id: "task_obj" },
          visibleTypeIds: [],
        }),
      ).toBe(false);
    });

    test("collection_obj без object_type_id — НЕ принимаем", () => {
      expect(
        shouldIncludeTypeInEdenListForLiveUpdate({
          typeId: "collection_obj",
          propsJson: {},
          visibleTypeIds: [],
        }),
      ).toBe(false);
    });

    test("collection_obj сам по себе как тип — НЕ принимаем (рекурсивные коллекции скрыты)", () => {
      expect(
        shouldIncludeTypeInEdenListForLiveUpdate({
          typeId: "collection_obj",
          propsJson: { object_type_id: "collection_obj" },
          visibleTypeIds: [],
        }),
      ).toBe(false);
    });
  });

  describe("visibleTypeIds непустой (пользователь выбрал конкретные типы)", () => {
    const visible = ["note_obj", "person_obj"];

    test("note_obj — принимаем (в списке)", () => {
      expect(
        shouldIncludeTypeInEdenListForLiveUpdate({
          typeId: "note_obj",
          propsJson: {},
          visibleTypeIds: visible,
        }),
      ).toBe(true);
    });

    test("person_obj — принимаем (в списке)", () => {
      expect(
        shouldIncludeTypeInEdenListForLiveUpdate({
          typeId: "person_obj",
          propsJson: {},
          visibleTypeIds: visible,
        }),
      ).toBe(true);
    });

    test("game_obj — НЕ принимаем (не в списке видимых типов)", () => {
      expect(
        shouldIncludeTypeInEdenListForLiveUpdate({
          typeId: "game_obj",
          propsJson: {},
          visibleTypeIds: visible,
        }),
      ).toBe(false);
    });

    test("collection_obj для видимого типа person_obj — принимаем", () => {
      expect(
        shouldIncludeTypeInEdenListForLiveUpdate({
          typeId: "collection_obj",
          propsJson: { object_type_id: "person_obj" },
          visibleTypeIds: visible,
        }),
      ).toBe(true);
    });

    test("collection_obj для невидимого типа game_obj — НЕ принимаем", () => {
      expect(
        shouldIncludeTypeInEdenListForLiveUpdate({
          typeId: "collection_obj",
          propsJson: { object_type_id: "game_obj" },
          visibleTypeIds: visible,
        }),
      ).toBe(false);
    });

    test("collection_obj для task_obj — НЕ принимаем (HIDDEN_EDEN_COLLECTION_TYPE_IDS)", () => {
      expect(
        shouldIncludeTypeInEdenListForLiveUpdate({
          typeId: "collection_obj",
          propsJson: { object_type_id: "task_obj" },
          visibleTypeIds: visible,
        }),
      ).toBe(false);
    });
  });
});
