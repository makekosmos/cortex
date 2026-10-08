import assert from "node:assert/strict";
import { test } from "node:test";
import { buildReleaseNotes } from "./publish-release.mjs";
import { previousReleaseBaseline, RELEASE_REPO } from "./release-plan.mjs";
import { RELEASE_BOM_FILE } from "./release-bom.mjs";

const FROM = "b".repeat(40);
const TO = "a".repeat(40);

// Same fake-run shape as release-plan.test.mjs: match the joined command
// line by prefix, unmatched commands fail loudly.
function fakeRun(handlers) {
  const calls = [];
  const run = (cmd, args) => {
    const line = [cmd, ...args].join(" ");
    calls.push(line);
    for (const [prefix, result] of handlers) {
      if (line.startsWith(prefix)) return { status: 0, stdout: "", stderr: "", ...result };
    }
    return { status: 127, stdout: "", stderr: `unexpected command: ${line}` };
  };
  return { run, calls };
}

const gitLogPrefix = `git log --no-merges --format=%h %s (%an) ${FROM}..${TO}`;

test("release notes list every commit with short hash, subject and author", () => {
  const { run, calls } = fakeRun([
    [
      `git log --no-merges`,
      {
        stdout:
          "abc1234 fix(engine): restart stalled supervisor (Jane Doe)\n" +
          "def5678 feat(manager): add tray menu (Jack)\n",
      },
    ],
  ]);
  const notes = buildReleaseNotes({ version: "0.10.7", fromCommit: FROM, toCommit: TO, run });
  assert.deepEqual(calls, [gitLogPrefix]);
  assert.equal(
    notes,
    "Mundus 0.10.7 installers + manifest.\n\n" +
      "- abc1234 fix(engine): restart stalled supervisor (Jane Doe)\n" +
      "- def5678 feat(manager): add tray menu (Jack)",
  );
});

test("release notes fail closed when git log fails", () => {
  const { run } = fakeRun([
    ["git log --no-merges", { status: 128, stderr: "fatal: bad revision" }],
  ]);
  assert.throws(
    () => buildReleaseNotes({ version: "0.10.7", fromCommit: FROM, toCommit: TO, run }),
    /git log .* failed \(128\): fatal: bad revision/,
  );
});

test("an empty commit range still produces the header", () => {
  const { run } = fakeRun([["git log --no-merges", { stdout: "" }]]);
  const notes = buildReleaseNotes({ version: "0.10.7", fromCommit: FROM, toCommit: TO, run });
  assert.match(notes, /^Mundus 0\.10\.7 installers \+ manifest\./);
});

test("previousReleaseBaseline resolves the published release source commit", () => {
  const bom = JSON.stringify({ release: { version: "0.10.6" }, source: { commit: FROM } });
  const { run } = fakeRun([
    [
      `gh api --paginate --slurp repos/${RELEASE_REPO}/releases`,
      {
        stdout: JSON.stringify([
          [{ tag_name: "v0.10.6", assets: [{ name: RELEASE_BOM_FILE, id: 42 }] }],
        ]),
      },
    ],
    ["gh api -H Accept: application/octet-stream", { stdout: bom }],
  ]);
  assert.deepEqual(previousReleaseBaseline({ run }), { tag: "v0.10.6", commit: FROM });
});
