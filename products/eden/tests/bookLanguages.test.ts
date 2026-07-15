import { describe, expect, test } from "vitest";
import {
  BOOK_LANGUAGE_OPTIONS,
  isBookLanguageField,
  normalizeBookLanguage,
} from "../src/lib/bookLanguages";

describe("book language normalization", () => {
  test("maps common codes and names to one selectable Russian value", () => {
    expect(["eng", "en", "English", "english", "Английский"].map(normalizeBookLanguage)).toEqual([
      "Английский",
      "Английский",
      "Английский",
      "Английский",
      "Английский",
    ]);
    expect(normalizeBookLanguage("pl")).toBe("Польский");
    expect(normalizeBookLanguage("xx-unknown")).toBe("xx-unknown");
  });

  test("only identifies the system book language schema", () => {
    expect(isBookLanguageField({ id: "language", options: BOOK_LANGUAGE_OPTIONS })).toBe(true);
    expect(isBookLanguageField({ id: "language", options: ["English", "German"] })).toBe(false);
  });
});
