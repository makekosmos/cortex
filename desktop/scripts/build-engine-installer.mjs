import { createHash } from "node:crypto";
import { existsSync, mkdirSync, readdirSync, readFileSync, rmSync, writeFileSync } from "node:fs";
import path from "node:path";
import { fileURLToPath } from "node:url";
import { execFileSync } from "node:child_process";
import { verifyEngineArchive } from "./engine-distribution.mjs";

const root = fileURLToPath(new URL("..", import.meta.url));
const payload = path.join(root, ".tmp", "engine.next");
const manifestPath = path.join(payload, "engine-manifest.json");
const manifest = JSON.parse(readFileSync(manifestPath, "utf8"));
verifyEngineArchive(path.join(payload, "Kosmos-Engine.zip"), manifest);
const sha256 = (file) => createHash("sha256").update(readFileSync(file)).digest("hex");
if (sha256(path.join(payload, "Kosmos-Engine.zip")) !== manifest.archive_sha256)
  throw new Error("Engine archive hash mismatch");

function findCompiler(directory) {
  if (!existsSync(directory)) return null;
  const executable = path.join(directory, "makensis.exe");
  if (existsSync(executable)) return executable;
  for (const entry of readdirSync(directory, { withFileTypes: true })) {
    if (entry.isDirectory()) {
      const found = findCompiler(path.join(directory, entry.name));
      if (found) return found;
    }
  }
  return null;
}
const cache = path.join(process.env.LOCALAPPDATA, "electron-builder", "Cache");
const compiler = process.env.MAKENSIS ?? findCompiler(cache);
if (!compiler) throw new Error("Set MAKENSIS to the NSIS makensis.exe executable");
const name = `Kosmos-Engine-Setup-${manifest.version}.exe`;
const trustedRelease = `https://github.com/makekosmos/desktop/releases/download/v${manifest.version}/`;
if (manifest.url !== `${trustedRelease}Kosmos-Engine-${manifest.version}.zip`)
  throw new Error("Engine archive URL must be the trusted versioned Desktop release");
const output = path.join(payload, name);
mkdirSync(path.dirname(output), { recursive: true });
rmSync(output, { force: true });
execFileSync(
  compiler,
  [
    "/V2",
    `/DVERSION=${manifest.version}`,
    `/DPAYLOAD=${payload}`,
    `/DOUTPUT=${output}`,
    "engine-installer.nsi",
  ],
  {
    cwd: path.join(root, "build"),
    stdio: "inherit",
    windowsHide: true,
  },
);
manifest.installer_url = `${trustedRelease}${name}`;
manifest.installer_sha256 = sha256(output);
manifest.installer_size = readFileSync(output).length;
// Engine and Desktop have independent release lines; do not use Desktop's latest tag.
manifest.channel_url = manifest.url.replace(/[^/]+$/, "Kosmos-Engine-manifest.json");
writeFileSync(manifestPath, JSON.stringify(manifest, null, 2) + "\n");
console.log(output);
