import assert from "node:assert/strict";
import fs from "node:fs";
import os from "node:os";
import path from "node:path";
import {
  runHeartVaultToArkObjectMigration,
  type ArkObjectLinkRecord,
  type ArkObjectRecord,
  type ArkObjectTypeRecord,
  type EdenArkObjectMigrationDeps,
  type Entry,
} from "../main/ark-object-migration.ts";
import type { NoteType } from "../src/lib/typedNotes.ts";

const vaultPath = "D:/tmp/eden-vault";
const operations: string[] = [];
const objects = new Map<string, ArkObjectRecord>();
const objectTypes = new Map<string, ArkObjectTypeRecord>();
const links = new Map<string, ArkObjectLinkRecord>();
const backupDir = fs.mkdtempSync(path.join(os.tmpdir(), "eden-note-migration-"));

function noteType(id: string): NoteType {
  return {
    id,
    name: id === "note_obj" ? "Note" : "Research Note",
    slug: id,
    icon: null,
    color: null,
    schema_json: JSON.stringify({
      fields: [
        {
          id: "summary",
          label: "Summary",
          kind: "long_text",
          required: false,
          visible: true,
        },
      ],
    }),
    header_template_json: JSON.stringify({ kind: "default", primaryFieldIds: ["summary"] }),
    ui_schema_json: JSON.stringify({ featured_fields: ["summary"], header_layout: "inline" }),
    created_at: Date.UTC(2026, 3, 25, 9, 0, 0),
    updated_at: Date.UTC(2026, 3, 25, 10, 0, 0),
  };
}

function entry(id: string, relatedNotes: string[] = [], typeId: string | null = null): Entry {
  return {
    id,
    title: `Entry ${id}`,
    content_json: JSON.stringify({ type: "doc", content: [{ type: "paragraph" }] }),
    created_at: Date.UTC(2026, 3, 25, 11, 0, 0),
    updated_at: Date.UTC(2026, 3, 25, 12, 0, 0),
    folder_id: null,
    type_id: typeId,
    header_layout: null,
    header_props_json: JSON.stringify({ related_notes: relatedNotes, summary: `Summary ${id}` }),
    schema_version: 1,
    deleted_at: null,
  };
}

const heartTypes = [noteType("note_obj"), noteType("research_note")];
const heartEntries = [
  entry("note-a", ["note-b"], "research_note"),
  entry("note-b", [], null),
];

const deps: EdenArkObjectMigrationDeps = {
  async listNoteTypes(path) {
    assert.equal(path, vaultPath);
    operations.push("listNoteTypes");
    return heartTypes;
  },
  async listEntries(path) {
    assert.equal(path, vaultPath);
    operations.push("listEntries");
    return heartEntries;
  },
  async listObjects() {
    operations.push("listObjects");
    return Array.from(objects.values());
  },
  async listObjectLinks() {
    operations.push("listObjectLinks");
    return Array.from(links.values());
  },
  async upsertObjectType(objectType) {
    operations.push(`upsertObjectType:${objectType.id}`);
    objectTypes.set(objectType.id, objectType);
    return true;
  },
  async upsertObject(object) {
    operations.push(`upsertObject:${object.id}`);
    objects.set(object.id, object);
    return true;
  },
  async upsertObjectLink(objectLink) {
    operations.push(`upsertObjectLink:${objectLink.id}`);
    assert.ok(objects.has(objectLink.sourceObjectId), "source object must exist before link migration");
    assert.ok(objects.has(objectLink.targetObjectId), "target object must exist before link migration");
    links.set(objectLink.id, objectLink);
    return true;
  },
  async deleteObjectLink(id) {
    operations.push(`deleteObjectLink:${id}`);
    links.delete(id);
    return true;
  },
};

const firstReport = await runHeartVaultToArkObjectMigration(deps, vaultPath, {
  backupDir,
  now: () => "2026-04-25T12:00:00.000Z",
});
const secondReport = await runHeartVaultToArkObjectMigration(deps, vaultPath, {
  backupDir,
  now: () => "2026-04-25T12:01:00.000Z",
});

assert.equal(firstReport.status, "success");
assert.equal(firstReport.typeMigratedCount, 2);
assert.equal(firstReport.objectMigratedCount, 2);
assert.equal(firstReport.linkMigratedCount, 1);
assert.ok(firstReport.backupPath);
assert.equal(fs.existsSync(firstReport.backupPath), true);
assert.equal(JSON.parse(fs.readFileSync(firstReport.backupPath, "utf8")).entries.length, 2);
assert.equal(secondReport.objectSkippedCount, 2);

assert.equal(objectTypes.get("note_obj")?.systemLocked, true);
assert.equal(objectTypes.get("research_note")?.name, "Research Note");
assert.equal(objects.get("note-a")?.typeId, "research_note");
assert.equal(objects.get("note-b")?.typeId, "note_obj");
assert.equal(objects.get("note-a")?.propsJson.summary, "Summary note-a");
assert.equal(links.has("note-a:related:note-b"), true);

const firstNoteBObjectIndex = operations.indexOf("upsertObject:note-b");
const firstLinkIndex = operations.indexOf("upsertObjectLink:note-a:related:note-b");
assert.ok(firstNoteBObjectIndex >= 0, "note-b should be migrated as an object");
assert.ok(firstLinkIndex > firstNoteBObjectIndex, "link migration must run after target object creation");
assert.equal(
  operations.filter((operation) => operation === "upsertObject:note-a").length,
  1,
  "migration should not duplicate existing note-a object on second run",
);
assert.equal(
  operations.filter((operation) => operation === "upsertObject:note-b").length,
  1,
  "migration should not duplicate existing note-b object on second run",
);

const failingObjects = new Map<string, ArkObjectRecord>();
const failingLinks = new Map<string, ArkObjectLinkRecord>();
const failingDeps: EdenArkObjectMigrationDeps = {
  async listNoteTypes() {
    return [noteType("note_obj")];
  },
  async listEntries() {
    return [entry("note-fail", ["missing-target"], null), entry("note-ok", [], null)];
  },
  async listObjects() {
    return Array.from(failingObjects.values());
  },
  async listObjectLinks() {
    return Array.from(failingLinks.values());
  },
  async upsertObjectType(objectType) {
    assert.equal(objectType.id, "note_obj");
    return true;
  },
  async upsertObject(object) {
    if (object.id === "note-fail") {
      throw new Error("object write failed");
    }
    failingObjects.set(object.id, object);
    return true;
  },
  async upsertObjectLink(objectLink) {
    throw new Error(`link write failed for ${objectLink.id}`);
  },
  async deleteObjectLink(id) {
    failingLinks.delete(id);
    return true;
  },
};

const failureReport = await runHeartVaultToArkObjectMigration(failingDeps, vaultPath, {
  backupDir: fs.mkdtempSync(path.join(os.tmpdir(), "eden-note-migration-failure-")),
  now: () => "2026-04-25T12:02:00.000Z",
});
assert.equal(failureReport.status, "partial_failure");
assert.equal(failureReport.objectMigratedCount, 1);
assert.equal(failureReport.objectFailedCount, 1);
assert.equal(failureReport.linkFailedCount, 1);
assert.equal(failureReport.errors.some((error) => error.id === "note-fail" && error.phase === "object"), true);
assert.equal(
  failureReport.errors.some((error) => error.id === "note-fail:related:missing-target" && error.phase === "link"),
  true,
);

console.log(JSON.stringify({
  status: "ok",
  objectTypes: Array.from(objectTypes.keys()).sort(),
  objects: Array.from(objects.keys()).sort(),
  links: Array.from(links.keys()).sort(),
  failureStatus: failureReport.status,
}));
