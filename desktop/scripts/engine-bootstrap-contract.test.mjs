import assert from "node:assert/strict";
import { existsSync, readFileSync } from "node:fs";
import path from "node:path";
import test from "node:test";

const desktopRoot = path.resolve(import.meta.dirname, "..");
const bootstrap = readFileSync(path.join(desktopRoot, "build", "install-engine.ps1"), "utf8");
const installer = readFileSync(path.join(desktopRoot, "build", "installer.nsi"), "utf8");

test("Desktop installs the Engine it was built with, from local resources", () => {
  // KOS-233: no separate publish/download step. The zip and manifest are the
  // same ones build-backend.mjs staged from this tree into .tmp/engine.next.
  assert.match(installer, /install-engine\.ps1/);
  assert.match(installer, /-Archive "\$INSTDIR\\resources\\Mundus Engine\.zip"/);
  assert.match(installer, /-Manifest "\$INSTDIR\\resources\\engine-manifest\.json"/);
  assert.doesNotMatch(
    bootstrap,
    /Invoke-WebRequest|installer_url|channel_url|Test-TrustedReleaseUrl/,
  );
  assert.match(bootstrap, /Compare-EngineVersion/);
});

test("install never downgrades an equal-or-newer verified Engine", () => {
  assert.match(bootstrap, /Test-InstalledEngine \$TargetRoot/);
  assert.match(
    bootstrap,
    /\$installedVersion -and \(Compare-EngineVersion \$installedVersion \$expected\.version\) -ge 0/,
  );
});

test("install takes over an existing standalone Mundus Engine registration", () => {
  assert.match(bootstrap, /function Invoke-EngineMigration/);
  assert.match(bootstrap, /Uninstall\\KosmosEngine/);
  assert.match(bootstrap, /Mundus Engine\.lnk/);
  // Snapshot before changing anything, restore it if the takeover fails —
  // never leave a half-migrated registration (KOS-134 pattern).
  assert.ok(bootstrap.indexOf("$snapshot") < bootstrap.indexOf("Remove-Item -LiteralPath $key"));
  assert.match(bootstrap, /rolled back/);
});

test("engine archive hash is verified before extraction", () => {
  assert.match(bootstrap, /archive_sha256/);
  assert.ok(bootstrap.indexOf("Get-EngineSha256 $Archive") < bootstrap.indexOf("Expand-Archive"));
  assert.equal(existsSync(path.join(desktopRoot, "build", "ensure-engine.ps1")), false);
  assert.equal(existsSync(path.join(desktopRoot, "build", "engine-installer.nsi")), false);
});
