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
    it("returns [] on empty storage", async () => {
      expect(await getSpaces()).toEqual([]);
    });

    it("returns [] when storage contains invalid JSON", async () => {
      localStorage.setItem("delphi.spaces", "{broken json!");

      expect(await getSpaces()).toEqual([]);
    });

    it("returns stored spaces", async () => {
      const spaces = [makeSpace("AAAA11112222")];

      localStorage.setItem("delphi.spaces", JSON.stringify(spaces));

      expect(await getSpaces()).toEqual(spaces);
    });
  });

  // -------------------------------------------------------------------------

  // saveSpace

  // -------------------------------------------------------------------------

  describe("saveSpace", () => {
    it("persists a new space to delphi.spaces key", async () => {
      const space = makeSpace("AAAA11112222");

      await saveSpace(space);

      const stored = JSON.parse(localStorage.getItem("delphi.spaces")!);

      expect(stored).toHaveLength(1);

      expect(stored[0].code).toBe("AAAA11112222");
    });

    it("prepends new space", async () => {
      await saveSpace(makeSpace("AAAA11112222", "First"));

      await saveSpace(makeSpace("BBBB33334444", "Second"));

      const stored = await getSpaces();

      expect(stored[0].code).toBe("BBBB33334444");

      expect(stored[1].code).toBe("AAAA11112222");
    });

    it("deduplicates by code", async () => {
      await saveSpace(makeSpace("AAAA11112222", "Original"));

      await saveSpace(makeSpace("AAAA11112222", "Updated"));

      const stored = await getSpaces();

      expect(stored).toHaveLength(1);

      expect(stored[0].name).toBe("Updated");
    });
  });

  // -------------------------------------------------------------------------

  // removeSpace

  // -------------------------------------------------------------------------

  describe("removeSpace", () => {
    it("removes matching code and persists the rest", async () => {
      await saveSpace(makeSpace("AAAA11112222"));

      await saveSpace(makeSpace("BBBB33334444"));

      await removeSpace("AAAA11112222");

      const stored = await getSpaces();

      expect(stored).toHaveLength(1);

      expect(stored[0].code).toBe("BBBB33334444");
    });

    it("no-op when code does not exist", async () => {
      await saveSpace(makeSpace("AAAA11112222"));

      await removeSpace("NONEXISTENT0");

      expect(await getSpaces()).toHaveLength(1);
    });
  });

  // -------------------------------------------------------------------------

  // renameSpace

  // -------------------------------------------------------------------------

  describe("renameSpace", () => {
    it("updates name in-place", async () => {
      await saveSpace(makeSpace("AAAA11112222", "Old"));

      const result = await renameSpace("AAAA11112222", "New Name");

      expect(result).toBe(true);

      expect((await getSpaces())[0].name).toBe("New Name");
    });

    it("returns false for unknown code", async () => {
      expect(await renameSpace("NONEXISTENT0", "Name")).toBe(false);
    });

    it("falls back to formatted code when name is blank", async () => {
      await saveSpace(makeSpace("AAAA11112222", "Old"));

      await renameSpace("AAAA11112222", "   ");

      expect((await getSpaces())[0].name).toBe(formatSpaceCode("AAAA11112222"));
    });
  });

  // -------------------------------------------------------------------------

  // getActiveSpace / setActiveSpace

  // -------------------------------------------------------------------------

  describe("getActiveSpace", () => {
    it("returns null when unset", async () => {
      expect(await getActiveSpace()).toBeNull();
    });

    it("returns stored code", async () => {
      await setActiveSpace("AAAA11112222");

      expect(await getActiveSpace()).toBe("AAAA11112222");
    });
  });

  describe("setActiveSpace", () => {
    it("stores the code", async () => {
      await setActiveSpace("AAAA11112222");

      expect(localStorage.getItem("delphi.active_space")).toBe("AAAA11112222");
    });

    it("removes the key when code is null", async () => {
      await setActiveSpace("AAAA11112222");

      await setActiveSpace(null);

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
    it("getSpaces with empty string in storage returns []", async () => {
      localStorage.setItem("delphi.spaces", "");

      expect(await getSpaces()).toEqual([]);
    });

    it("saveSpace on corrupt storage recovers", async () => {
      localStorage.setItem("delphi.spaces", "NOT_JSON");

      // saveSpace calls getSpaces() internally which returns [] on corrupt data

      await saveSpace(makeSpace("AAAA11112222"));

      expect(await getSpaces()).toHaveLength(1);
    });
  });
});
