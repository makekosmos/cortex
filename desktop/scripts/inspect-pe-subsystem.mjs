import { execFileSync } from "node:child_process";
import { createHash } from "node:crypto";
import { existsSync, readFileSync } from "node:fs";
import path from "node:path";
import { fileURLToPath } from "node:url";

const UNINSTALL_ROOT = "HKCU\\Software\\Microsoft\\Windows\\CurrentVersion\\Uninstall";

function normalizeRoot(value) {
  return path
    .resolve(value)
    .replace(/[\\/]+$/, "")
    .toLowerCase();
}

export function parseRegistryRecords(output) {
  const records = [];
  let current = null;
  for (const line of output.split(/\r?\n/)) {
    const key = line.match(/^(HKEY_[^\s]+)$/i);
    if (key) {
      current = { key: key[1] };
      records.push(current);
      continue;
    }
    const value = line.match(
      /^\s*(DisplayName|DisplayVersion|InstallLocation|UninstallString)\s+REG_\S+\s+(.*)$/i,
    );
    if (current && value) current[value[1]] = value[2].trim();
  }
  return records;
}

export function readUninstallProvenance(expectedVersion) {
  if (process.platform !== "win32")
    throw new Error("Windows registry provenance is unavailable on this platform");
  const output = execFileSync("reg.exe", ["query", UNINSTALL_ROOT, "/s"], {
    encoding: "utf8",
    windowsHide: true,
    stdio: ["ignore", "pipe", "pipe"],
  });
  const record = parseRegistryRecords(output).find(
    (item) =>
      item.DisplayName?.toLowerCase().includes("mundus") && item.DisplayVersion === expectedVersion,
  );
  if (!record?.UninstallString) {
    throw new Error(`No Mundus uninstall record found for version ${expectedVersion}`);
  }
  if (!record.InstallLocation) {
    const uninstallExe = record.UninstallString.match(/^"([^"]+)"/)?.[1];
    if (uninstallExe) record.InstallLocation = path.dirname(uninstallExe);
  }
  if (!record.InstallLocation) throw new Error("Uninstall record has no install root");
  return record;
}

export function readPeSubsystem(file) {
  const bytes = readFileSync(file);
  if (bytes.readUInt16LE(0) !== 0x5a4d) throw new Error(`${file}: missing MZ header`);
  const peOffset = bytes.readUInt32LE(0x3c);
  if (bytes.toString("ascii", peOffset, peOffset + 4) !== "PE\0\0")
    throw new Error(`${file}: missing PE header`);
  const optionalOffset = peOffset + 24;
  const magic = bytes.readUInt16LE(optionalOffset);
  if (magic !== 0x10b && magic !== 0x20b)
    throw new Error(`${file}: unsupported PE optional header`);
  const subsystem = bytes.readUInt16LE(optionalOffset + 68);
  return subsystem === 2 ? "WINDOWS" : subsystem === 3 ? "CONSOLE" : `UNKNOWN(${subsystem})`;
}

export function inspectInstall(installRoot, expectedVersion) {
  const root = normalizeRoot(installRoot);
  const provenance = readUninstallProvenance(expectedVersion);
  if (normalizeRoot(provenance.InstallLocation) !== root) {
    throw new Error(
      `Install root mismatch: registry=${provenance.InstallLocation}, argument=${installRoot}`,
    );
  }
  if (!provenance.UninstallString.toLowerCase().includes(root)) {
    throw new Error("UninstallString does not point to the supplied install root");
  }
  console.log(
    `PROVENANCE DisplayName=${provenance.DisplayName} DisplayVersion=${provenance.DisplayVersion}`,
  );
  console.log(`PROVENANCE InstallLocation=${provenance.InstallLocation}`);
  console.log(`PROVENANCE UninstallString=${provenance.UninstallString}`);
  for (const name of [
    "components/manager/Mundus Manager.exe",
    "components/agenda/Agenda.exe",
    "components/memoria/Memoria.exe",
    "components/dictation/Dictation.exe",
  ]) {
    const file = path.join(installRoot, "resources", name);
    if (!existsSync(file)) continue;
    const hash = createHash("sha256").update(readFileSync(file)).digest("hex");
    console.log(`FILE ${name} SHA256=${hash} SUBSYSTEM ${readPeSubsystem(file)}`);
  }
}

if (
  process.argv[1] &&
  path.resolve(process.argv[1]) === path.resolve(fileURLToPath(import.meta.url))
) {
  const rootIndex = process.argv.indexOf("--install-root");
  const versionIndex = process.argv.indexOf("--expected-version");
  const installRoot = rootIndex >= 0 ? process.argv[rootIndex + 1] : "";
  const expectedVersion = versionIndex >= 0 ? process.argv[versionIndex + 1] : "";
  if (!installRoot || !expectedVersion) {
    console.error(
      "Usage: node inspect-pe-subsystem.mjs --install-root <path> --expected-version <version>",
    );
    process.exitCode = 2;
  } else {
    try {
      inspectInstall(installRoot, expectedVersion);
    } catch (error) {
      console.error(error instanceof Error ? error.message : String(error));
      process.exitCode = 2;
    }
  }
}
