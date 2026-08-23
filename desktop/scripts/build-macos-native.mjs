#!/usr/bin/env node
import { mkdirSync } from "node:fs";
import { spawnSync } from "node:child_process";
import path from "node:path";
import { fileURLToPath } from "node:url";

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
