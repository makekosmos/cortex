/**
 * AC1: Space Manager unit tests.
 *
 * Tests all localStorage-based functions and deriveSpaceId.
 * Runs in jsdom environment (default vitest config) which provides localStorage.
 */

import { beforeEach, describe, expect, it } from "vitest";

import {
  type Space,
  deriveSpaceId,
  formatSpaceCode,
  getActiveSpace,
  getSpaces,
  removeSpace,
  renameSpace,
  saveSpace,
  setActiveSpace,
} from "../space-manager";

// ---------------------------------------------------------------------------

// Helpers

// ---------------------------------------------------------------------------

function makeSpace(code: string, name?: string): Space {
  return {
    code,

    name: name ?? `Space ${code}`,

    createdAt: new Date().toISOString(),
  };
}

// ---------------------------------------------------------------------------

// Tests

// ---------------------------------------------------------------------------

describe("Space Manager", () => {
  beforeEach(() => {
    localStorage.clear();
  });

  // -------------------------------------------------------------------------

  // getSpaces

  // -------------------------------------------------------------------------

  describe("getSpaces", () => {
    it("returns [] on empty storage", () => {
      expect(getSpaces()).toEqual([]);
    });

    it("returns [] when storage contains invalid JSON", () => {
      localStorage.setItem("delphi.spaces", "{broken json!");

      expect(getSpaces()).toEqual([]);
    });

    it("returns stored spaces", () => {
      const spaces = [makeSpace("AAAA11112222")];

      localStorage.setItem("delphi.spaces", JSON.stringify(spaces));

      expect(getSpaces()).toEqual(spaces);
    });
  });

  // -------------------------------------------------------------------------

  // saveSpace

  // -------------------------------------------------------------------------

  describe("saveSpace", () => {
    it("persists a new space to delphi.spaces key", () => {
      const space = makeSpace("AAAA11112222");

      saveSpace(space);

      const stored = JSON.parse(localStorage.getItem("delphi.spaces")!);

      expect(stored).toHaveLength(1);

      expect(stored[0].code).toBe("AAAA11112222");
    });

    it("prepends new space", () => {
      saveSpace(makeSpace("AAAA11112222", "First"));

      saveSpace(makeSpace("BBBB33334444", "Second"));

      const stored = getSpaces();

      expect(stored[0].code).toBe("BBBB33334444");

      expect(stored[1].code).toBe("AAAA11112222");
    });

    it("deduplicates by code", () => {
      saveSpace(makeSpace("AAAA11112222", "Original"));

      saveSpace(makeSpace("AAAA11112222", "Updated"));

      const stored = getSpaces();

      expect(stored).toHaveLength(1);

      expect(stored[0].name).toBe("Updated");
    });
  });

  // -------------------------------------------------------------------------

  // removeSpace

  // -------------------------------------------------------------------------

  describe("removeSpace", () => {
    it("removes matching code and persists the rest", () => {
      saveSpace(makeSpace("AAAA11112222"));

      saveSpace(makeSpace("BBBB33334444"));

      removeSpace("AAAA11112222");

      const stored = getSpaces();

      expect(stored).toHaveLength(1);

      expect(stored[0].code).toBe("BBBB33334444");
    });

    it("no-op when code does not exist", () => {
      saveSpace(makeSpace("AAAA11112222"));

      removeSpace("NONEXISTENT0");

      expect(getSpaces()).toHaveLength(1);
    });
  });

  // -------------------------------------------------------------------------

  // renameSpace

  // -------------------------------------------------------------------------

  describe("renameSpace", () => {
    it("updates name in-place", () => {
      saveSpace(makeSpace("AAAA11112222", "Old"));

      const result = renameSpace("AAAA11112222", "New Name");

      expect(result).toBe(true);

      expect(getSpaces()[0].name).toBe("New Name");
    });

    it("returns false for unknown code", () => {
      expect(renameSpace("NONEXISTENT0", "Name")).toBe(false);
    });

    it("falls back to formatted code when name is blank", () => {
      saveSpace(makeSpace("AAAA11112222", "Old"));

      renameSpace("AAAA11112222", "   ");

      expect(getSpaces()[0].name).toBe(formatSpaceCode("AAAA11112222"));
    });
  });

  // -------------------------------------------------------------------------

  // getActiveSpace / setActiveSpace

  // -------------------------------------------------------------------------

  describe("getActiveSpace", () => {
    it("returns null when unset", () => {
      expect(getActiveSpace()).toBeNull();
    });

    it("returns stored code", () => {
      setActiveSpace("AAAA11112222");

      expect(getActiveSpace()).toBe("AAAA11112222");
    });
  });

  describe("setActiveSpace", () => {
    it("stores the code", () => {
      setActiveSpace("AAAA11112222");

      expect(localStorage.getItem("delphi.active_space")).toBe("AAAA11112222");
    });

    it("removes the key when code is null", () => {
      setActiveSpace("AAAA11112222");

      setActiveSpace(null);

      expect(localStorage.getItem("delphi.active_space")).toBeNull();
    });
  });

  // -------------------------------------------------------------------------

  // deriveSpaceId

  // -------------------------------------------------------------------------

  describe("deriveSpaceId", () => {
    it("produces a 16-hex-char string", async () => {
      const id = await deriveSpaceId("AAAA11112222");

      expect(id).toMatch(/^[0-9a-f]{16}$/);
    });

    it("matches a known SHA-256 test vector", async () => {
      // SHA-256("AAAA11112222") -> compute expected

      // We compute the reference ourselves: the function normalizes to uppercase, strips dashes.

      // Input: "AAAA11112222" (already uppercase, no dashes)

      const encoder = new TextEncoder();

      const data = encoder.encode("AAAA11112222");

      const hashBuffer = await crypto.subtle.digest("SHA-256", data);

      const hashArray = Array.from(new Uint8Array(hashBuffer));

      const expected = hashArray

        .map((b) => b.toString(16).padStart(2, "0"))

        .join("")

        .slice(0, 16);

      const id = await deriveSpaceId("AAAA11112222");

      expect(id).toBe(expected);
    });

    it("normalizes code before hashing (strips dashes, uppercases)", async () => {
      const a = await deriveSpaceId("aaaa-1111-2222");

      const b = await deriveSpaceId("AAAA11112222");

      expect(a).toBe(b);
    });
  });

  // -------------------------------------------------------------------------

  // Edge cases

  // -------------------------------------------------------------------------

  describe("edge cases", () => {
    it("getSpaces with empty string in storage returns []", () => {
      localStorage.setItem("delphi.spaces", "");

      expect(getSpaces()).toEqual([]);
    });

    it("saveSpace on corrupt storage recovers", () => {
      localStorage.setItem("delphi.spaces", "NOT_JSON");

      // saveSpace calls getSpaces() internally which returns [] on corrupt data

      saveSpace(makeSpace("AAAA11112222"));

      expect(getSpaces()).toHaveLength(1);
    });
  });
});
