import assert from "node:assert/strict";
import { existsSync, readdirSync } from "node:fs";
import { readFile } from "node:fs/promises";
import path from "node:path";
import { test } from "node:test";
import { releaseBuildIdentity } from "./release-build-env.mjs";

const script = await readFile(path.join(import.meta.dirname, "build-desktop.mjs"), "utf8");
const preflight = await readFile(path.join(import.meta.dirname, "release-preflight.mjs"), "utf8");
const backend = await readFile(path.join(import.meta.dirname, "build-backend.mjs"), "utf8");
const releaseVersions = JSON.parse(
  await readFile(path.join(import.meta.dirname, "..", "release-versions.json"), "utf8"),
);

test("release build verifies locally and leaves publishing to the receipt consumer", () => {
  assert.match(script, /ensureNsis\(/);
  assert.match(script, /createReceipt\(/);
  assert.match(script, /writeReceipt\(/);
  assert.doesNotMatch(script, /publishRelease\(|release create|electron-builder/);
  assert.match(script, /emitProvenance\(/);
  assert.match(script, /Preflight: source, BOM, and pins/);
  assert.match(script, /--dry-run/);
  assert.doesNotMatch(script, /--clobber/);
  assert.match(script, /deriveReleaseBom\(/);
  assert.doesNotMatch(script, /--bom|RELEASE_BOM"|loadReleaseBom/);
  assert.match(preflight, /release builds require a clean tracked and source worktree/);
});

test("makensis reads installer.nsi as UTF-8, so its Russian strings survive any code page", () => {
  assert.match(script, /"\/INPUTCHARSET",\s*"UTF8",/);
});

test("Engine ships from the same build and version as the GUI (KOS-233)", () => {
  // There is exactly one version source: desktop/release-versions.json.
  // desktop/engine-version.json must not exist, and nothing downloads a
  // separately published Mundus-Engine-*.zip during a build.
  assert.equal(existsSync(path.join(import.meta.dirname, "..", "engine-version.json")), false);
  assert.ok(releaseVersions.win);
  assert.doesNotMatch(backend, /MUNDUS_ENGINE_RELEASE/);
  assert.doesNotMatch(backend, /consumeEngineArtifacts/);
  assert.doesNotMatch(backend, /releases\/download\/v/);
  assert.match(backend, /releaseBuildIdentity\(shellRoot\)/);
  assert.match(backend, /buildEnginePayload\(stageDir, engineDir/);
  assert.doesNotMatch(script, /copyEngineRelease\(/);
  assert.doesNotMatch(script, /copyEngineManifest\(/);
});

// KOS-278: the Engine updater read MUNDUS_PRODUCT_VERSION while the build set
// only MUNDUS_ENGINE_VERSION, so released 0.10.0 reported the crate's 0.1.0 and
// offered 0.10.0 as an update forever. Every compile-time version variable a
// shipped binary reads must be the one both builds inject.
test("shipped binaries read the product version only from the variable the builds set", async () => {
  const repo = path.join(import.meta.dirname, "..", "..");
  const components = await readFile(
    path.join(import.meta.dirname, "build-package-components.mjs"),
    "utf8",
  );
  const buildEnv = await readFile(path.join(import.meta.dirname, "release-build-env.mjs"), "utf8");
  const macos = await readFile(path.join(import.meta.dirname, "build-macos-native.mjs"), "utf8");
  assert.match(buildEnv, /MUNDUS_PRODUCT_VERSION: productVersion/);
  assert.match(buildEnv, /MUNDUS_ENGINE_SOURCE_COMMIT: sourceCommit/);
  assert.match(components, /releaseBuildIdentity\(root\)\.env/);
  assert.match(macos, /releaseBuildIdentity\(cortexRoot\)/);
  // A bare cargo build has no injected version. Both readers say "dev"
  // rather than the crate's 0.1.0.
  for (const file of [
    "runtime/crates/engine-base/src/build_info.rs",
    "manager-gpui/src/views/about.rs",
  ]) {
    const source = await readFile(path.join(repo, file), "utf8");
    assert.match(source, /"dev"/, file);
    assert.doesNotMatch(source, /CARGO_PKG_VERSION/, file);
  }
  const readers = [];
  for (const dir of ["runtime/src", "runtime/crates", "manager-gpui/src", "core/crates"]) {
    for (const entry of readdirSync(path.join(repo, dir), { recursive: true })) {
      if (!entry.endsWith(".rs")) continue;
      const source = await readFile(path.join(repo, dir, entry), "utf8");
      for (const [, name] of source.matchAll(/option_env!\("([A-Z0-9_]*VERSION[A-Z0-9_]*)"\)/g))
        readers.push(`${dir}/${entry.replaceAll("\\", "/")}: ${name}`);
    }
  }
  assert.ok(readers.length > 0, "expected the product version to be read somewhere");
  for (const reader of readers) assert.match(reader, /: MUNDUS_PRODUCT_VERSION$/, reader);
  const repoRoot = path.join(import.meta.dirname, "..", "..");
  const identity = releaseBuildIdentity(repoRoot);
  assert.equal(identity.productVersion, releaseVersions.win);
  assert.equal(identity.env.MUNDUS_PRODUCT_VERSION, identity.productVersion);
  assert.equal(identity.env.MUNDUS_ENGINE_SOURCE_COMMIT, identity.sourceCommit);
  assert.match(identity.sourceCommit, /^[0-9a-f]{40}$/);
});

// KOS-306: Defender's first-sight ML flagged the 0.10.1 installer. These
// contracts pin the fixes: no script host, no per-image kills, unpacked
// engine payload, VERSIONINFO on the installer and every shipped exe, and a
// static Defender gate in the build.
test("installer.nsi contains no powershell, no ExecutionPolicy and no taskkill", async () => {
  const nsi = await readFile(
    path.join(import.meta.dirname, "..", "build", "installer.nsi"),
    "utf8",
  );
  assert.doesNotMatch(nsi, /powershell/i);
  assert.doesNotMatch(nsi, /ExecutionPolicy/);
  assert.doesNotMatch(nsi, /taskkill/i);
});

test("the staged installer payload contains no .ps1", async () => {
  // KOS-306: enforced at build time inside stageInstaller — pin both the
  // check and the absence of staged scripts in the tree.
  assert.match(script, /assertNoPowerShellPayload\(stage\)/);
  const buildDir = path.join(import.meta.dirname, "..", "build");
  for (const entry of readdirSync(buildDir)) {
    assert.ok(!entry.endsWith(".ps1"), `staged script left behind: ${entry}`);
  }
});

test("the installer and every shipped exe get VERSIONINFO", async () => {
  const nsi = await readFile(
    path.join(import.meta.dirname, "..", "build", "installer.nsi"),
    "utf8",
  );
  assert.match(nsi, /VIProductVersion "\$\{VERSION\}\.0"/);
  for (const key of [
    "ProductName",
    "CompanyName",
    "FileDescription",
    "FileVersion",
    "ProductVersion",
    "LegalCopyright",
  ]) {
    assert.match(nsi, new RegExp(`VIAddVersionKey "${key}"`));
  }
  // The post-build check fails the build when a shipped exe reports empty
  // CompanyName/ProductName/FileDescription or a FileVersion that is not
  // the product version — installer included.
  assert.match(script, /assertVersionInfo\(outFile, version\)/);
  assert.match(script, /assertApplicationManifest\(outFile\)/);
  assert.match(script, /mundus-engine\.exe/);
  assert.match(script, /MANAGER_EXE/);
});

// KOS-306 round 2: the VERSIONINFO brand strings live in three places —
// pe-version-info (exes), desktop/scripts/brand.mjs and installer.nsi. Pin
// them to each other so a rebrand never splits them.
test("the VERSIONINFO brand strings share one source across pe-version-info, brand.mjs and installer.nsi", async () => {
  const helper = await readFile(
    path.join(
      import.meta.dirname,
      "..",
      "..",
      "runtime",
      "crates",
      "pe-version-info",
      "src",
      "lib.rs",
    ),
    "utf8",
  );
  const brand = await readFile(path.join(import.meta.dirname, "brand.mjs"), "utf8");
  const runtimeBrand = await readFile(
    path.join(
      import.meta.dirname,
      "..",
      "..",
      "runtime",
      "crates",
      "engine-base",
      "src",
      "brand.rs",
    ),
    "utf8",
  );
  const nsi = await readFile(
    path.join(import.meta.dirname, "..", "build", "installer.nsi"),
    "utf8",
  );
  assert.match(helper, /pub const PRODUCT_NAME: &str = "Mundus"/);
  assert.match(helper, /pub const COMPANY_NAME: &str = "Kazui"/);
  assert.match(helper, /pub const LEGAL_COPYRIGHT: &str = "Copyright \(C\) Kazui"/);
  assert.match(brand, /PRODUCT_NAME = "Mundus"/);
  assert.match(brand, /PUBLISHER = "Kazui"/);
  assert.match(runtimeBrand, /PRODUCT_NAME: &str = "Mundus"/);
  assert.match(nsi, /!define APP_NAME "Mundus"/);
  assert.match(nsi, /!define PUBLISHER "Kazui"/);
  assert.match(nsi, /VIAddVersionKey "ProductName" "\$\{APP_NAME\}"/);
  assert.match(nsi, /VIAddVersionKey "CompanyName" "\$\{PUBLISHER\}"/);
  assert.match(nsi, /VIAddVersionKey "LegalCopyright" "Copyright \(C\) Kazui"/);
  // The build-time check compares the stamped strings to the same constants.
  assert.match(script, /ProductName !== "Mundus"/);
  assert.match(script, /LegalCopyright !== "Copyright \(C\) Kazui"/);
});

test("the build runs a static Defender scan on the finished installer", () => {
  assert.match(script, /MpCmdRun\.exe/);
  assert.match(
    script,
    /"-Scan",\s*"-ScanType",\s*"3",\s*"-File",\s*outFile,\s*"-DisableRemediation"/,
  );
});
