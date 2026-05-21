// Golden tests для todoFilterService — baseline poведения TS-impl.
// Это контракт, который должен совпадать с Rust-port'ом.

import { describe, expect, test } from "bun:test";
import { SmartList } from "../src/types/task";
import { filterTodos, countTodos, countAll } from "../src/services/filters/todoFilterService";
import { FIXTURES, EXPECTED, EXPECTED_COUNTS, TODAY_ISO } from "./filterService.fixtures";

// `isDateToday` в impl читает `new Date()` напрямую — для детерминистических
// тестов мокаем Date.now() / Date constructor через global Date override.
// Bun test использует jest-style mocks — но проще через monkey-patch.
const realDateNow = Date.now;
const realDateConstructor = Date;

function freezeDate(iso: string) {
  const fixed = new Date(iso + "T12:00:00.000Z").getTime();
  globalThis.Date = class extends realDateConstructor {
    constructor(...args: ConstructorParameters<typeof Date>) {
      if (args.length === 0) {
        super(fixed);
      } else {
        // @ts-expect-error variadic
        super(...args);
      }
    }
    static now() {
      return fixed;
    }
  } as unknown as DateConstructor;
}

function restoreDate() {
  globalThis.Date = realDateConstructor;
  globalThis.Date.now = realDateNow;
}

describe("todoFilterService — golden parity baseline", () => {
  for (const [listKey, expectedIds] of Object.entries(EXPECTED)) {
    test(`filterTodos(${listKey}) returns expected ids in order`, () => {
      freezeDate(TODAY_ISO);
      try {
        const result = filterTodos(listKey as SmartList, FIXTURES);
        expect(result.map((t) => t.id)).toEqual(expectedIds);
      } finally {
        restoreDate();
      }
    });
  }

  test("countAll matches expected counts", () => {
    freezeDate(TODAY_ISO);
    try {
      const counts = countAll(FIXTURES);
      for (const [list, expected] of Object.entries(EXPECTED_COUNTS)) {
        expect(counts[list as SmartList]).toBe(expected);
      }
    } finally {
      restoreDate();
    }
  });

  test("countTodos per-list matches countAll[list]", () => {
    freezeDate(TODAY_ISO);
    try {
      for (const list of Object.values(SmartList)) {
        expect(countTodos(list, FIXTURES)).toBe(EXPECTED_COUNTS[list]);
      }
    } finally {
      restoreDate();
    }
  });

  test("empty input → empty output / zero counts", () => {
    freezeDate(TODAY_ISO);
    try {
      for (const list of Object.values(SmartList)) {
        expect(filterTodos(list, [])).toEqual([]);
        expect(countTodos(list, [])).toBe(0);
      }
    } finally {
      restoreDate();
    }
  });

  test("filterTodos не мутирует input array", () => {
    freezeDate(TODAY_ISO);
    try {
      const copy = FIXTURES.slice();
      filterTodos(SmartList.Inbox, FIXTURES);
      expect(FIXTURES.map((t) => t.id)).toEqual(copy.map((t) => t.id));
    } finally {
      restoreDate();
    }
  });
});
