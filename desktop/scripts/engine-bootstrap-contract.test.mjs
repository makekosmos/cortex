import assert from "node:assert/strict";
import { existsSync, readFileSync } from "node:fs";
import path from "node:path";
import test from "node:test";

const desktopRoot = path.resolve(import.meta.dirname, "..");
const installer = readFileSync(path.join(desktopRoot, "build", "installer.nsi"), "utf8");
const installerSource = readFileSync(
  path.join(desktopRoot, "..", "runtime", "src", "installer", "install.rs"),
  "utf8",
);

test("Desktop installs the Engine it was built with, from local resources", () => {
  // KOS-233: no separate publish/download step. The staged payload dir and
  // manifest are the ones build-backend.mjs staged from this tree into
  // .tmp/engine.next; KOS-306 runs the install in the staged exe itself.
  assert.match(installer, /!define ENGINE_STAGED "\$INSTDIR\\resources\\engine"/);
  assert.match(
    installer,
    /mundus-engine\.exe" install --manifest "\$\{ENGINE_STAGED\}\\engine-manifest\.json" --target-root "\$\{ENGINE_ROOT\}"/,
  );
  assert.doesNotMatch(
    installer,
    /Invoke-WebRequest|installer_url|channel_url|Test-TrustedReleaseUrl/,
  );
});

test("install keeps a newer or identical verified Engine, replaces a same-version rebuild", () => {
  // The monotonic rule and the same-build comparison live in the `install`
  // subcommand now — pin the behaviour where it is implemented.
  assert.match(installerSource, /installed_version\(&options\.target_root\)/);
  assert.match(installerSource, /\*installed > expected\.version/);
  assert.match(installerSource, /manifest::same_build/);
});

test("install takes over an existing standalone Mundus Engine registration", () => {
  // MIGRATION(KOS-267): remove after 2026-11-01.
  const legacy = readFileSync(
    path.join(desktopRoot, "..", "runtime", "src", "installer", "legacy.rs"),
    "utf8",
  );
  assert.match(legacy, /migrate_standalone_registration/);
  assert.match(legacy, /Uninstall\\KosmosEngine/);
  assert.match(legacy, /Kosmos Engine\.lnk/);
  // Snapshot before changing anything, restore if the takeover fails —
  // never leave a half-migrated registration (KOS-134 pattern).
  assert.ok(legacy.indexOf("snapshot") < legacy.indexOf("delete_tree"));
  assert.match(legacy, /rolled back/);
});

test("the payload ships unpacked and every file is hash-verified before install", () => {
  // KOS-306: no zip, no Expand-Archive — the manifest's per-file sha256
  // covers tampering the way archive_sha256 did.
  assert.match(installerSource, /manifest::verify_file\(&payload_dir/);
  assert.doesNotMatch(installer, /ENGINE_ARCHIVE|Mundus Engine\.zip/);
  assert.equal(existsSync(path.join(desktopRoot, "build", "ensure-engine.ps1")), false);
  assert.equal(existsSync(path.join(desktopRoot, "build", "engine-installer.nsi")), false);
});
