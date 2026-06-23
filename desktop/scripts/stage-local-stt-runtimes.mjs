#!/usr/bin/env node
import { cpSync, existsSync, mkdirSync, rmSync } from "node:fs";
import path from "node:path";
import { fileURLToPath } from "node:url";

const __dirname = path.dirname(fileURLToPath(import.meta.url));
const shellRoot = path.resolve(__dirname, "..");
const repoRoot = path.resolve(shellRoot, "../..");

const runtimes = [
  {
    name: "whisper.cpp",
    source: path.join(repoRoot, ".tmp/whisper.cpp/build-cpu/bin"),
    destination: path.join(shellRoot, "build/local-stt/whisper.cpp/Release"),
  },
  {
    name: "whisper.cpp-vulkan",
    source: path.join(repoRoot, ".tmp/whisper.cpp/build-vulkan/bin"),
    destination: path.join(shellRoot, "build/local-stt/whisper.cpp-vulkan/Release"),
  },
];

function hasRuntime(dir) {
  return existsSync(path.join(dir, "whisper-cli.exe"));
}

let staged = 0;
for (const runtime of runtimes) {
  if (hasRuntime(runtime.destination)) {
    console.log(`[stage-local-stt] ${runtime.name}: already staged`);
    continue;
  }
  if (!hasRuntime(runtime.source)) {
    console.error(
      `[stage-local-stt] ${runtime.name}: missing ${runtime.source}\n` +
        "Build whisper.cpp CPU/Vulkan runtimes first, or stage build/local-stt manually.",
    );
    process.exit(1);
  }
  rmSync(runtime.destination, { recursive: true, force: true });
  mkdirSync(runtime.destination, { recursive: true });
  for (const file of [
    "whisper-cli.exe",
    "ggml.dll",
    "ggml-base.dll",
    "ggml-cpu.dll",
    "whisper.dll",
  ]) {
    cpSync(path.join(runtime.source, file), path.join(runtime.destination, file));
  }
  const vulkanDll = path.join(runtime.source, "ggml-vulkan.dll");
  if (existsSync(vulkanDll)) {
    cpSync(vulkanDll, path.join(runtime.destination, "ggml-vulkan.dll"));
  }
  staged += 1;
  console.log(`[stage-local-stt] ${runtime.name}: staged`);
}

console.log(`[stage-local-stt] done (${staged} copied)`);
