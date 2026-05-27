import crypto from "node:crypto";
import { existsSync, mkdirSync, readFileSync, rmSync, statSync } from "node:fs";
import path from "node:path";
import { execSync } from "node:child_process";
import { entriesFromDir, writeZip } from "./zip-utils.mjs";

export function buildNativeRelease(id, manifest, repoRoot, logPrefix = "native:package") {
  const pkg = manifest.native?.cargoPackage ?? id;
  console.log(`[${logPrefix}] cargo build --release -p ${pkg}`);
  execSync(`cargo build --release -p ${pkg}`, { cwd: repoRoot, stdio: "inherit" });
}

export function nativeExecutablePath(id, manifest, repoRoot) {
  const pkg = manifest.native?.cargoPackage ?? id;
  const exeName = process.platform === "win32" ? `${pkg}.exe` : pkg;
  return path.join(repoRoot, "target", "release", exeName);
}

export function packageExtensionKext(id, manifest, options) {
  const { extensionsRoot, repoRoot, outDir, logPrefix = "ext:package" } = options;
  const extDir = path.join(extensionsRoot, id);
  const version = manifest.version ?? "0.0.0";
  mkdirSync(outDir, { recursive: true });
  const kextPath = path.join(outDir, `${id}-${version}.kext`);
  if (existsSync(kextPath)) rmSync(kextPath, { force: true });

  const entries = [];
  entries.push({
    name: "manifest.json",
    data: readFileSync(path.join(extDir, "manifest.json")),
  });

  if (manifest.icon) {
    const iconPath = path.join(extDir, manifest.icon);
    if (existsSync(iconPath)) {
      entries.push({ name: manifest.icon, data: readFileSync(iconPath) });
    } else {
      console.warn(`[${logPrefix}] icon указан в manifest, но файл не найден: ${iconPath}`);
    }
  }

  const readmePath = path.join(extDir, "README.md");
  if (existsSync(readmePath)) {
    entries.push({ name: "README.md", data: readFileSync(readmePath) });
  }

  if (manifest.kind === "native") {
    const exePath = nativeExecutablePath(id, manifest, repoRoot);
    if (!existsSync(exePath)) {
      throw new Error(`native executable не найден: ${exePath}`);
    }
    const executableRel = manifest.native?.executable;
    if (!executableRel) {
      throw new Error(`native.executable не указан для ${id}`);
    }
    entries.push({ name: executableRel.replace(/\\/g, "/"), data: readFileSync(exePath) });
  } else {
    const distDir = path.join(extDir, "dist");
    if (!existsSync(distDir)) {
      throw new Error(`dist/ не найден для ${id} - build:extensions падал?`);
    }
    for (const e of entriesFromDir(distDir)) {
      entries.push({ name: `dist/${e.name}`, data: e.data });
    }
  }

  writeZip(kextPath, entries);
  return kextPath;
}

export function sha256File(p) {
  const h = crypto.createHash("sha256");
  h.update(readFileSync(p));
  return h.digest("hex");
}

export function fileSize(p) {
  return statSync(p).size;
}
