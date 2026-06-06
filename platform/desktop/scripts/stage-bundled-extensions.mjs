#!/usr/bin/env node
import { cpSync, existsSync, mkdirSync, rmSync } from "node:fs";
import path from "node:path";
import { fileURLToPath } from "node:url";
import { listRepoExtensionEntries } from "./repo-extension-roots.mjs";

const __filename = fileURLToPath(import.meta.url);
const __dirname = path.dirname(__filename);
const desktopRoot = path.resolve(__dirname, "..");
const repoRoot = path.resolve(desktopRoot, "..", "..");
const outRoot = path.join(desktopRoot, ".tmp", "bundled-extensions", "extensions");

rmSync(outRoot, { recursive: true, force: true });
mkdirSync(outRoot, { recursive: true });

const staged = [];
for (const entry of listRepoExtensionEntries(repoRoot)) {
  if (entry.manifest.kind === "native") continue;
  const dist = path.join(entry.dir, "dist");
  if (!existsSync(dist)) {
    console.warn(`[stage:bundled-extensions] skip ${entry.id}: dist/ not found`);
    continue;
  }

  const target = path.join(outRoot, entry.id);
  mkdirSync(target, { recursive: true });
  cpSync(path.join(entry.dir, "manifest.json"), path.join(target, "manifest.json"));
  cpSync(dist, path.join(target, "dist"), { recursive: true });

  if (entry.manifest.icon) {
    const iconPath = path.join(entry.dir, entry.manifest.icon);
    if (existsSync(iconPath)) cpSync(iconPath, path.join(target, entry.manifest.icon));
  }

  const readme = path.join(entry.dir, "README.md");
  if (existsSync(readme)) cpSync(readme, path.join(target, "README.md"));
  staged.push(`${entry.id} (${entry.rootName}/${entry.folder})`);
}

console.log(`[stage:bundled-extensions] staged ${staged.length}: ${staged.join(", ")}`);
