import assert from "node:assert/strict";
import { mkdir, mkdtemp, rm, writeFile } from "node:fs/promises";
import os from "node:os";
import path from "node:path";
import test from "node:test";
import { missingDocPaths } from "./check-doc-paths.mjs";

async function fixture(files) {
  const root = await mkdtemp(path.join(os.tmpdir(), "cortex-doc-paths-"));
  for (const [relative, content] of Object.entries(files)) {
    const target = path.join(root, relative);
    await mkdir(path.dirname(target), { recursive: true });
    await writeFile(target, content, "utf8");
  }
  return root;
}

test("accepts documents whose paths exist", async () => {
  const root = await fixture({
    "AGENTS.md": "Rules live in `scripts/check-doc-paths.mjs` and in\n[the ladder](README.md).\n",
    "README.md": "See `docs/architecture.md`.\n",
    "docs/architecture.md": "# Architecture\n",
    "scripts/check-doc-paths.mjs": "// check\n",
  });
  try {
    assert.deepEqual(await missingDocPaths(root), []);
  } finally {
    await rm(root, { recursive: true, force: true });
  }
});

test("reports a documented path that no longer exists", async () => {
  const root = await fixture({
    "AGENTS.md": "The gate is `scripts/check-source-size.mjs`.\n",
    "docs/old.md": "Details in `docs/removed-decision.md`.\n",
  });
  try {
    assert.deepEqual(await missingDocPaths(root), [
      "AGENTS.md: scripts/check-source-size.mjs",
      "docs/old.md: docs/removed-decision.md",
    ]);
  } finally {
    await rm(root, { recursive: true, force: true });
  }
});

test("ignores bare file names, globs, urls and generated output trees", async () => {
  const root = await fixture({
    "AGENTS.md": [
      "`dev.mjs` means scripts/dev.mjs.",
      "`docs/*.md` is a glob.",
      "`https://example.test/x.md` is a link out.",
      "`release/release-receipt.v2.json` is build output.",
      "`.audit/receipts.md` is a local receipt.",
      "",
    ].join("\n"),
  });
  try {
    assert.deepEqual(await missingDocPaths(root), []);
  } finally {
    await rm(root, { recursive: true, force: true });
  }
});

test("keeps dated experiment records frozen", async () => {
  const root = await fixture({
    "docs/experiments/2026-01-01-old-layout.md": "Moved to `runtime/src/gone.rs`.\n",
  });
  try {
    assert.deepEqual(await missingDocPaths(root), []);
  } finally {
    await rm(root, { recursive: true, force: true });
  }
});
