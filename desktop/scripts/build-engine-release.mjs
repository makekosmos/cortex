#!/usr/bin/env node
import { spawnSync } from "node:child_process";
import path from "node:path";
import { fileURLToPath } from "node:url";
import { copyEngineManifest, copyEngineRelease } from "./engine-distribution.mjs";
import { cleanBuildIntermediates } from "./runtime-staging.mjs";

const shellRoot = path.resolve(fileURLToPath(new URL("..", import.meta.url)));
const version = process.env.KOSMOS_ENGINE_VERSION;
if (!version) {
  console.error(
    "[build-engine-release] KOSMOS_ENGINE_VERSION is required for an independent engine release",
  );
  process.exit(2);
}
const result = spawnSync(process.execPath, [path.join(shellRoot, "scripts", "build-backend.mjs")], {
  cwd: shellRoot,
  stdio: "inherit",
  windowsHide: true,
  env: { ...process.env, KOSMOS_ENGINE_RELEASE: "1", KOSMOS_ENGINE_VERSION: version },
});
if ((result.status ?? 1) !== 0) process.exit(result.status ?? 1);
try {
  copyEngineRelease(shellRoot, version);
  copyEngineManifest(shellRoot, version);
} finally {
  cleanBuildIntermediates(shellRoot);
}
