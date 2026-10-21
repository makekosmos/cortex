import assert from "node:assert/strict";
import { mkdir, mkdtemp, rm, writeFile } from "node:fs/promises";
import os from "node:os";
import path from "node:path";
import test from "node:test";

import { collectCommandInventory } from "./audit-ark-core-storage-census.mjs";

test("the command census records typed execution boundaries and ignores test code", async (t) => {
  const root = await mkdtemp(path.join(os.tmpdir(), "ark-core-command-census-"));
  t.after(() => rm(root, { recursive: true, force: true }));
  const sourceRoot = path.join(root, "core", "crates", "ark-core", "src");
  const commands = path.join(sourceRoot, "db", "commands.rs");
  const tests = path.join(sourceRoot, "db", "commands_tests.rs");
  await mkdir(path.dirname(commands), { recursive: true });
  await writeFile(
    commands,
    [
      "// impl DbCommand for CommentOnly",
      "impl DbCommand for WriteThing {}",
      "impl DbQuery for ReadThing {}",
      "impl DbMaintenanceCommand for RestoreThing {}",
      "#[cfg(test)] mod tests { impl DbCommand for InlineFixture {} }",
    ].join("\n"),
  );
  await writeFile(tests, "impl DbCommand for FileFixture {}\n");

  const inventory = await collectCommandInventory(sourceRoot, root);
  assert.deepEqual(
    inventory.map(({ kind, name, boundary }) => ({ kind, name, boundary })),
    [
      {
        kind: "DbCommand",
        name: "WriteThing",
        boundary: "immediate write transaction",
      },
      {
        kind: "DbMaintenanceCommand",
        name: "RestoreThing",
        boundary: "exclusive maintenance connection",
      },
      {
        kind: "DbQuery",
        name: "ReadThing",
        boundary: "deferred read transaction",
      },
    ],
  );
});
