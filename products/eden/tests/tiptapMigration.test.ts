import { describe, expect, test } from "bun:test";

import { writeEntryMarkdown, writeEntryTiptapDoc } from "../src/editor-cm/content";
import {
  analyzeTipTapMigration,
  buildTipTapMigrationContentJson,
  countMigratableEntries,
  createTipTapMigrationSnapshot,
  migrateEntriesToTiptapJson,
  rollbackTipTapMigration,
} from "../src/editor-tiptap/migration";

function makeEntry(id: string, contentJson: string): Entry {
  return {
    id,
    title: `Entry ${id}`,
    content_json: contentJson,
    created_at: 1000,
    updated_at: 2000,
    folder_id: null,
    type_id: "note_obj",
    header_layout: null,
    header_props_json: "{}",
    schema_version: 1,
    deleted_at: null,
  };
}

describe("tiptap migration", () => {
  test("buildTipTapMigrationContentJson migrates only markdown-wrapper entries", () => {
    const markdownEntry = JSON.stringify(writeEntryMarkdown("# Title\n\n- [ ] Task"));
    const tiptapEntry = JSON.stringify(
      writeEntryTiptapDoc({
        type: "doc",
        content: [{ type: "paragraph", content: [{ type: "text", text: "done" }] }],
      }),
    );

    expect(buildTipTapMigrationContentJson(markdownEntry)).not.toBeNull();
    expect(buildTipTapMigrationContentJson(tiptapEntry)).toBeNull();
    expect(buildTipTapMigrationContentJson("{not-json")).toBeNull();
  });

  test("countMigratableEntries counts only entries that still need conversion", () => {
    const entries = [
      makeEntry("a", JSON.stringify(writeEntryMarkdown("one"))),
      makeEntry("b", JSON.stringify(writeEntryMarkdown("two"))),
      makeEntry(
        "c",
        JSON.stringify(
          writeEntryTiptapDoc({
            type: "doc",
            content: [{ type: "paragraph", content: [{ type: "text", text: "three" }] }],
          }),
        ),
      ),
    ];

    expect(countMigratableEntries(entries)).toBe(2);
  });

  test("analyzeTipTapMigration reports migratable, already migrated, deleted, and invalid entries", () => {
    const markdown = makeEntry("a", JSON.stringify(writeEntryMarkdown("one")));
    const tiptap = makeEntry(
      "b",
      JSON.stringify(
        writeEntryTiptapDoc({
          type: "doc",
          content: [{ type: "paragraph", content: [{ type: "text", text: "two" }] }],
        }),
      ),
    );
    const deleted = {
      ...makeEntry("c", JSON.stringify(writeEntryMarkdown("deleted"))),
      deleted_at: 3000,
    };
    const invalid = makeEntry("d", "{not-json");
    const unsupportedTiptap = makeEntry(
      "e",
      JSON.stringify({ type: "tiptap", version: 1, doc: { type: "text" } }),
    );

    expect(analyzeTipTapMigration([markdown, tiptap, deleted, invalid, unsupportedTiptap])).toEqual(
      {
        total: 5,
        migratable: 1,
        skipped: 4,
        skippedDeleted: 1,
        skippedAlreadyTiptap: 1,
        skippedInvalid: 1,
        skippedUnsupported: 1,
      },
    );
  });

  test("createTipTapMigrationSnapshot keeps originals only for entries that can migrate", () => {
    const markdown = makeEntry("a", JSON.stringify(writeEntryMarkdown("one")));
    const tiptap = makeEntry(
      "b",
      JSON.stringify(
        writeEntryTiptapDoc({
          type: "doc",
          content: [{ type: "paragraph", content: [{ type: "text", text: "two" }] }],
        }),
      ),
    );
    const deleted = {
      ...makeEntry("c", JSON.stringify(writeEntryMarkdown("deleted"))),
      deleted_at: 3000,
    };

    const snapshot = createTipTapMigrationSnapshot([markdown, tiptap, deleted]);

    expect(snapshot).toEqual([markdown]);
    expect(snapshot[0]).not.toBe(markdown);
  });

  test("migrateEntriesToTiptapJson saves migrated entries and skips the rest", async () => {
    const entries = [
      makeEntry("a", JSON.stringify(writeEntryMarkdown("alpha"))),
      makeEntry(
        "b",
        JSON.stringify(
          writeEntryTiptapDoc({
            type: "doc",
            content: [{ type: "paragraph", content: [{ type: "text", text: "beta" }] }],
          }),
        ),
      ),
    ];
    const saved: Entry[] = [];
    const drafted: Entry[] = [];

    const result = await migrateEntriesToTiptapJson(
      entries,
      async (entry) => {
        saved.push(entry);
        return { ok: true, entryId: entry.id };
      },
      (entry) => {
        drafted.push(entry);
      },
    );

    expect(result).toMatchObject({ converted: 1, skipped: 1, failed: 0 });
    expect(result.updatedIds).toEqual(["a"]);
    expect(result.skippedEntries).toEqual([{ id: "b", reason: "already_tiptap" }]);
    expect(result.correlationId).toBeTruthy();
    expect(saved).toHaveLength(1);
    expect(drafted).toHaveLength(1);
    expect(saved[0]?.id).toBe("a");
    expect(saved[0]?.updated_at).toBeGreaterThan(entries[0]!.updated_at);
    expect(saved[0]?.content_json).toContain('"type":"tiptap"');
  });

  test("migrateEntriesToTiptapJson reports failed ids and errors", async () => {
    const entries = [
      makeEntry("a", JSON.stringify(writeEntryMarkdown("alpha"))),
      makeEntry("b", JSON.stringify(writeEntryMarkdown("beta"))),
      makeEntry("c", JSON.stringify(writeEntryMarkdown("gamma"))),
    ];

    const result = await migrateEntriesToTiptapJson(entries, async (entry) => {
      if (entry.id === "a") return { ok: true, entryId: entry.id };
      if (entry.id === "b") return { ok: false, reason: "duplicate_title", message: "duplicate" };
      throw new Error("disk offline");
    });

    expect(result.converted).toBe(1);
    expect(result.skipped).toBe(0);
    expect(result.failed).toBe(2);
    expect(result.failures).toEqual([
      { id: "b", stage: "persist", error: "duplicate" },
      { id: "c", stage: "persist", error: "disk offline" },
    ]);
  });

  test("migrateEntriesToTiptapJson does not update draft when save fails", async () => {
    const entries = [makeEntry("a", JSON.stringify(writeEntryMarkdown("alpha")))];
    const drafted: Entry[] = [];

    const result = await migrateEntriesToTiptapJson(
      entries,
      async () => ({ ok: false, message: "write rejected" }),
      (entry) => {
        drafted.push(entry);
      },
    );

    expect(result).toMatchObject({ converted: 0, skipped: 0, failed: 1 });
    expect(result.failures).toEqual([{ id: "a", stage: "persist", error: "write rejected" }]);
    expect(drafted).toHaveLength(0);
  });

  test("migrateEntriesToTiptapJson emits progress with correlation id", async () => {
    const entries = [
      makeEntry("a", JSON.stringify(writeEntryMarkdown("alpha"))),
      makeEntry("b", "{not-json"),
    ];
    const progress: Array<{
      phase: string;
      processed: number;
      correlationId: string;
    }> = [];

    const result = await migrateEntriesToTiptapJson(
      entries,
      async (entry) => ({ ok: true, entryId: entry.id }),
      undefined,
      {
        correlationId: "run-1",
        onProgress: (event) => {
          progress.push({
            phase: event.phase,
            processed: event.processed,
            correlationId: event.correlationId,
          });
        },
      },
    );

    expect(result).toMatchObject({
      converted: 1,
      skipped: 1,
      failed: 0,
      correlationId: "run-1",
      skippedEntries: [{ id: "b", reason: "invalid_json" }],
    });
    expect(progress.at(-1)).toEqual({
      phase: "done",
      processed: 2,
      correlationId: "run-1",
    });
  });

  test("rollbackTipTapMigration restores snapshot entries through save path", async () => {
    const snapshot = [
      makeEntry("a", JSON.stringify(writeEntryMarkdown("alpha"))),
      makeEntry("b", JSON.stringify(writeEntryMarkdown("beta"))),
    ];
    const saved: Entry[] = [];
    const drafted: Entry[] = [];

    const result = await rollbackTipTapMigration(
      snapshot,
      async (entry) => {
        saved.push(entry);
        return { ok: true, entryId: entry.id };
      },
      (entry) => {
        drafted.push(entry);
      },
    );

    expect(result).toMatchObject({ restored: 2, skipped: 0, failed: 0 });
    expect(result.restoredIds).toEqual(["a", "b"]);
    expect(saved.map((entry) => entry.id)).toEqual(["a", "b"]);
    expect(drafted.map((entry) => entry.id)).toEqual(["a", "b"]);
    expect(saved[0]?.content_json).toBe(snapshot[0]?.content_json);
    expect(saved[0]?.updated_at).toBeGreaterThan(snapshot[0]!.updated_at);
  });

  test("rollbackTipTapMigration reports save failures without drafting failed entries", async () => {
    const snapshot = [
      makeEntry("a", JSON.stringify(writeEntryMarkdown("alpha"))),
      makeEntry("b", JSON.stringify(writeEntryMarkdown("beta"))),
    ];
    const drafted: Entry[] = [];

    const result = await rollbackTipTapMigration(
      snapshot,
      async (entry) => {
        if (entry.id === "a") return { ok: false, message: "stale entry" };
        return { ok: true, entryId: entry.id };
      },
      (entry) => {
        drafted.push(entry);
      },
    );

    expect(result).toMatchObject({
      restored: 1,
      skipped: 0,
      failed: 1,
      failures: [{ id: "a", stage: "rollback", error: "stale entry" }],
    });
    expect(drafted.map((entry) => entry.id)).toEqual(["b"]);
  });
});
