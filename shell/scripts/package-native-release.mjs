#!/usr/bin/env node

import {
  existsSync,
  mkdirSync,
  readFileSync,
  readdirSync,
  rmSync,
  statSync,
  writeFileSync,
} from "node:fs";
import path from "node:path";
import { fileURLToPath } from "node:url";
import { spawnSync } from "node:child_process";
import {
  buildNativeRelease,
  fileSize,
  nativeExecutablePath,
  packageExtensionKext,
  sha256File,
} from "./extension-package-utils.mjs";

const __filename = fileURLToPath(import.meta.url);
const __dirname = path.dirname(__filename);
const SHELL_ROOT = path.resolve(__dirname, "..");
const REPO_ROOT = path.resolve(SHELL_ROOT, "..");
const EXTENSIONS_ROOT = path.join(REPO_ROOT, "extensions");

function die(msg) {
  console.error(`[native:package] ${msg}`);
  process.exit(1);
}

function readManifest(id) {
  const p = path.join(EXTENSIONS_ROOT, id, "manifest.json");
  if (!existsSync(p)) die(`manifest.json не найден: ${p}`);
  return JSON.parse(readFileSync(p, "utf8"));
}

function resolveMakensis() {
  if (process.env.MAKENSIS_PATH && existsSync(process.env.MAKENSIS_PATH)) {
    return process.env.MAKENSIS_PATH;
  }
  const probe = spawnSync("makensis", ["/VERSION"], { stdio: ["ignore", "pipe", "pipe"] });
  if (probe.status === 0) return "makensis";
  const cached = findCachedMakensis();
  if (cached) return cached;
  die(
    "makensis.exe не найден. Установи NSIS и добавь makensis в PATH, либо укажи MAKENSIS_PATH=C:\\Path\\To\\makensis.exe.",
  );
}

function findCachedMakensis() {
  const roots = [process.env.LOCALAPPDATA, process.env.APPDATA]
    .filter(Boolean)
    .map((base) => path.join(base, "electron-builder", "Cache", "nsis"));
  for (const root of roots) {
    const found = findFile(root, "makensis.exe", 4);
    if (found) return found;
  }
  return null;
}

function findFile(dir, fileName, depth) {
  if (depth < 0 || !existsSync(dir)) return null;
  for (const entry of readdirSync(dir)) {
    const full = path.join(dir, entry);
    const stat = statSync(full);
    if (stat.isFile() && entry.toLowerCase() === fileName.toLowerCase()) {
      return full;
    }
    if (stat.isDirectory()) {
      const found = findFile(full, fileName, depth - 1);
      if (found) return found;
    }
  }
  return null;
}

function nsisString(value) {
  return String(value).replace(/\\/g, "\\\\").replace(/"/g, '$\\"').replace(/\r?\n/g, " ");
}

function writeInstallerScript(id, manifest, exePath, workDir, outDir) {
  const version = manifest.version ?? "0.0.0";
  const productName = manifest.name ?? id;
  const exeName = `${productName}.exe`;
  const installerName = `${productName} Setup ${version}.exe`;
  const registryKey = "Software\\Microsoft\\Windows\\CurrentVersion\\Uninstall\\com.kazui.akasha";
  const classesKey = "Software\\Classes";
  const scriptPath = path.join(workDir, "installer.nsi");
  const installerPath = path.join(outDir, installerName);

  const script = `
Unicode true
RequestExecutionLevel user
SetCompressor /SOLID lzma

Name "${nsisString(productName)}"
OutFile "${nsisString(installerPath)}"
InstallDir "$LOCALAPPDATA\\Programs\\${nsisString(productName)}"
BrandingText "${nsisString(productName)}"

!define PRODUCT_NAME "${nsisString(productName)}"
!define PRODUCT_VERSION "${nsisString(version)}"
!define PRODUCT_PUBLISHER "Kazui"
!define EXE_NAME "${nsisString(exeName)}"
!define UNINSTALL_KEY "${nsisString(registryKey)}"

Section "Install"
  SetShellVarContext current
  SetOutPath "$INSTDIR"
  File /oname=$INSTDIR\\${nsisString(exeName)} "${nsisString(exePath)}"

  CreateDirectory "$SMPROGRAMS\\${nsisString(productName)}"
  CreateShortcut "$SMPROGRAMS\\${nsisString(productName)}\\${nsisString(productName)}.lnk" "$INSTDIR\\${nsisString(exeName)}"
  CreateShortcut "$DESKTOP\\${nsisString(productName)}.lnk" "$INSTDIR\\${nsisString(exeName)}"

  WriteUninstaller "$INSTDIR\\Uninstall.exe"
  WriteRegStr HKCU "${nsisString(registryKey)}" "DisplayName" "${nsisString(productName)}"
  WriteRegStr HKCU "${nsisString(registryKey)}" "DisplayVersion" "${nsisString(version)}"
  WriteRegStr HKCU "${nsisString(registryKey)}" "Publisher" "Kazui"
  WriteRegStr HKCU "${nsisString(registryKey)}" "InstallLocation" "$INSTDIR"
  WriteRegStr HKCU "${nsisString(registryKey)}" "DisplayIcon" "$INSTDIR\\${nsisString(exeName)}"
  WriteRegStr HKCU "${nsisString(registryKey)}" "UninstallString" "$INSTDIR\\Uninstall.exe"
  WriteRegDWORD HKCU "${nsisString(registryKey)}" "NoModify" 1
  WriteRegDWORD HKCU "${nsisString(registryKey)}" "NoRepair" 1

  WriteRegStr HKCU "${classesKey}\\.epub" "" "Akasha.epub"
  WriteRegStr HKCU "${classesKey}\\Akasha.epub" "" "EPUB Book"
  WriteRegStr HKCU "${classesKey}\\Akasha.epub\\DefaultIcon" "" "$INSTDIR\\${nsisString(exeName)},0"
  WriteRegStr HKCU "${classesKey}\\Akasha.epub\\shell\\open\\command" "" '$\\"$INSTDIR\\${nsisString(exeName)}$\\" --open $\\"%1$\\"'
  System::Call 'shell32::SHChangeNotify(i 0x08000000, i 0, i 0, i 0)'
SectionEnd

Section "Uninstall"
  SetShellVarContext current
  Delete "$DESKTOP\\${nsisString(productName)}.lnk"
  Delete "$SMPROGRAMS\\${nsisString(productName)}\\${nsisString(productName)}.lnk"
  RMDir "$SMPROGRAMS\\${nsisString(productName)}"

  Delete "$INSTDIR\\${nsisString(exeName)}"
  Delete "$INSTDIR\\Uninstall.exe"
  RMDir "$INSTDIR"

  DeleteRegKey HKCU "${classesKey}\\Akasha.epub"
  DeleteRegValue HKCU "${classesKey}\\.epub" ""
  DeleteRegKey HKCU "${nsisString(registryKey)}"
  System::Call 'shell32::SHChangeNotify(i 0x08000000, i 0, i 0, i 0)'
SectionEnd
`.trimStart();

  writeFileSync(scriptPath, script, "utf8");
  return { scriptPath, installerPath };
}

function runMakensis(makensis, scriptPath) {
  console.log(`[native:package] makensis ${scriptPath}`);
  const result = spawnSync(makensis, [scriptPath], { stdio: "inherit" });
  if (result.status !== 0) {
    die(`makensis failed (exit ${result.status})`);
  }
}

const args = process.argv.slice(2);
const noBuild = args.includes("--no-build");
const id = args.find((arg) => !arg.startsWith("--"));
if (!id) die("usage: native:package <extension-id> [--no-build]");

const manifest = readManifest(id);
if (manifest.kind !== "native") {
  die(`${id} is not a native extension`);
}

const outDir = path.join(SHELL_ROOT, "release", "native", id);
const workDir = path.join(SHELL_ROOT, ".tmp", "native-package", id);
rmSync(workDir, { recursive: true, force: true });
mkdirSync(outDir, { recursive: true });
mkdirSync(workDir, { recursive: true });

if (!noBuild) {
  buildNativeRelease(id, manifest, REPO_ROOT, "native:package");
}

const kextPath = packageExtensionKext(id, manifest, {
  extensionsRoot: EXTENSIONS_ROOT,
  repoRoot: REPO_ROOT,
  outDir,
  logPrefix: "native:package",
});
const exePath = nativeExecutablePath(id, manifest, REPO_ROOT);
const { scriptPath, installerPath } = writeInstallerScript(id, manifest, exePath, workDir, outDir);
runMakensis(resolveMakensis(), scriptPath);

console.log(
  `[native:package] wrote ${kextPath} (${fileSize(kextPath)}B, sha256 ${sha256File(kextPath).slice(0, 12)}...)`,
);
console.log(
  `[native:package] wrote ${installerPath} (${fileSize(installerPath)}B, sha256 ${sha256File(installerPath).slice(0, 12)}...)`,
);
