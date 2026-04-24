import { describe, expect, it } from "vitest";

import { normalizeNumberedPlaceholders } from "./sqlite";

describe("normalizeNumberedPlaceholders", () => {
  it("duplicates reused numbered parameters for positional sqlite bindings", () => {
    expect(
      normalizeNumberedPlaceholders(
        "SELECT * FROM games WHERE name LIKE ?1 OR exe_name LIKE ?1",
        ["%doom%"],
      ),
    ).toEqual({
      sql: "SELECT * FROM games WHERE name LIKE ? OR exe_name LIKE ?",
      params: ["%doom%", "%doom%"],
    });
  });

  it("keeps numbered parameter order by placeholder occurrence", () => {
    expect(
      normalizeNumberedPlaceholders(
        "INSERT INTO games (id, name, date_added) VALUES (?1, ?2, ?1)",
        ["game-1", "Doom"],
      ),
    ).toEqual({
      sql: "INSERT INTO games (id, name, date_added) VALUES (?, ?, ?)",
      params: ["game-1", "Doom", "game-1"],
    });
  });

  it("throws a clear error for missing numbered parameters", () => {
    expect(() => normalizeNumberedPlaceholders("SELECT ?2", ["only-one"])).toThrow(
      "Missing SQLite parameter ?2",
    );
  });
});
