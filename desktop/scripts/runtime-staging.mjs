import { copyFileSync, existsSync, mkdirSync } from "node:fs";
import path from "node:path";

export const RUNTIME_BINARIES = [
  "kepler-backend",
  "ark-core-rpc",
  "kepler-focus-helper",
  "kepler-focus-svc",
];

export function effectiveCargoTargetDir(shellRoot, repoRoot, configuredTargetDir) {
  return configuredTargetDir
    ? path.resolve(shellRoot, configuredTargetDir)
    : path.join(repoRoot, "target");
}

export function stageRuntimeBinaries(releaseDir, stageDir, platform = process.platform) {
  const suffix = platform === "win32" ? ".exe" : "";
  mkdirSync(stageDir, { recursive: true });
  for (const binary of RUNTIME_BINARIES) {
    const source = path.join(releaseDir, `${binary}${suffix}`);
    if (!existsSync(source)) {
      throw new Error(`freshly built binary missing: ${source}`);
    }
    copyFileSync(source, path.join(stageDir, `${binary}${suffix}`));
  }
}

export function stageRuntimeBinary(binary, releaseDir, stageDir, platform = process.platform) {
  const suffix = platform === "win32" ? ".exe" : "";
  mkdirSync(stageDir, { recursive: true });
  const source = path.join(releaseDir, `${binary}${suffix}`);
  if (!existsSync(source)) throw new Error(`freshly built binary missing: ${source}`);
  copyFileSync(source, path.join(stageDir, `${binary}${suffix}`));
}
