import { expect, test } from "../test-support/node-test.mjs";
import { readFileSync } from "node:fs";
import path from "node:path";
import { fileURLToPath } from "node:url";
import { listRepoExtensionEntries } from "./repo-extension-roots.mjs";

const cortexRoot = path.resolve(path.dirname(fileURLToPath(import.meta.url)), "..", "..");
const productsRoot = path.resolve(cortexRoot, "..");

test("standalone repositories are discoverable from Makekosmos", () => {
  const ids = new Set(listRepoExtensionEntries(productsRoot).map((entry) => entry.id));
  for (const id of ["eden", "delphi", "arrancador", "dictation"]) expect(ids).toContain(id);
});

for (const [repo, appId] of Object.entries({
  memoria: "com.kosmos.memoria",
  agenda: "com.kosmos.agenda",
  arcadia: "com.kosmos.arcadia",
  dictation: "com.kosmos.dictation",
})) {
  test(`${repo} declares its immutable appId`, () => {
    const manifest = JSON.parse(
      readFileSync(path.join(productsRoot, repo, "manifest.json"), "utf8"),
    );
    expect(manifest.appId).toBe(appId);
  });
}
