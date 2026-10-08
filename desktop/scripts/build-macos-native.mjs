#!/usr/bin/env node
// macOS product build: Swift dictation helpers, then the Engine and Manager
// with the product version baked in (release-build-env.mjs). Off macOS this
// is a no-op — the Windows installer never ships these binaries, and a
// Windows host has no swiftc. The desktop `build` script still invokes this
// step so a Mac checkout produces them without a second recipe.
import { mkdirSync } from "node:fs";
import { spawnSync } from "node:child_process";
import path from "node:path";
import { fileURLToPath } from "node:url";
import { releaseBuildIdentity } from "./release-build-env.mjs";

const __dirname = path.dirname(fileURLToPath(import.meta.url));
const desktopRoot = path.resolve(__dirname, "..");
const cortexRoot = path.resolve(desktopRoot, "..");

if (process.platform !== "darwin") {
  process.exit(0);
}

const sourceDir = path.join(cortexRoot, "runtime", "native", "macos");
const outDir = path.join(desktopRoot, ".tmp", "native", "macos");
const moduleCacheDir = path.join(desktopRoot, ".tmp", "swift-module-cache");
mkdirSync(outDir, { recursive: true });
mkdirSync(moduleCacheDir, { recursive: true });

const helpers = [
  {
    out: "hotkey-hold-monitor",
    sources: ["hotkey-hold-monitor.swift"],
    frameworks: ["CoreGraphics", "AppKit", "Carbon"],
  },
  {
    out: "capture-hotkey",
    sources: ["capture-hotkey.swift"],
    frameworks: ["CoreGraphics", "AppKit", "Carbon"],
  },
  {
    out: "audio-capturer",
    sources: ["audio-capturer.swift"],
    frameworks: ["AVFoundation", "Foundation"],
  },
  {
    out: "microphone-access",
    sources: ["microphone-access.swift"],
    frameworks: ["AVFoundation"],
  },
  {
    out: "speech-recognizer",
    sources: ["speech-recognizer.swift"],
    frameworks: ["Speech", "AVFoundation"],
  },
  {
    out: "input-monitoring-request",
    sources: ["input-monitoring-request.swift"],
    frameworks: ["CoreGraphics"],
  },
  {
    out: "get-selected-text",
    sources: ["get-selected-text.swift"],
    frameworks: ["Foundation", "ApplicationServices", "AppKit"],
  },
  {
    out: "paste-text",
    sources: ["paste-text.swift"],
    frameworks: ["AppKit", "ApplicationServices", "CoreGraphics"],
  },
];

for (const helper of helpers) {
  const args = [
    "-O",
    "-module-cache-path",
    moduleCacheDir,
    "-o",
    path.join(outDir, helper.out),
    ...helper.sources.map((source) => path.join(sourceDir, source)),
  ];
  for (const framework of helper.frameworks) {
    args.push("-framework", framework);
  }
  const result = spawnSync("swiftc", args, { stdio: "inherit" });
  if (result.status !== 0) {
    process.exit(result.status ?? 1);
  }
}

// The Swift helpers are not the product binary. Manager's About screen and
// the Engine both read MUNDUS_PRODUCT_VERSION at compile time; without it
// they report the crate's 0.1.0. Windows injects the same identity from
// release-build-env.mjs. This is a host build: no windows-gui subsystem and
// no cross target.
const { env } = releaseBuildIdentity(cortexRoot);
function cargo(args, cwd) {
  const result = spawnSync("cargo", args, { cwd, stdio: "inherit", env });
  if ((result.status ?? 1) !== 0) process.exit(result.status ?? 1);
}
cargo(
  [
    "build",
    "--release",
    "--locked",
    "--manifest-path",
    path.join(cortexRoot, "Cargo.toml"),
    "--bin",
    "mundus-engine",
  ],
  desktopRoot,
);
cargo(
  [
    "build",
    "--release",
    "--locked",
    "--manifest-path",
    path.join(cortexRoot, "manager-gpui", "Cargo.toml"),
  ],
  path.join(cortexRoot, "manager-gpui"),
);
