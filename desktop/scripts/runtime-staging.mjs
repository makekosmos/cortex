import {
  closeSync,
  copyFileSync,
  existsSync,
  mkdirSync,
  openSync,
  readFileSync,
  rmSync,
  writeFileSync,
} from "node:fs";
import path from "node:path";

// Engine privileged ops live inside mundus-engine.exe itself
// (`mundus-engine privileged install|uninstall|run-service`) — no separate
// helper exes ship in the distribution.
export const RUNTIME_BINARIES = ["mundus-engine"];

export function acquireBuildLock(root) {
  const lock = path.join(root, ".tmp", "build.active.lock");
  mkdirSync(path.dirname(lock), { recursive: true });
  if (existsSync(lock)) {
    let pid = 0;
    try {
      pid = Number(readFileSync(lock, "utf8"));
    } catch {}
    let active = false;
    if (pid > 0) {
      try {
        process.kill(pid, 0);
        active = true;
      } catch {}
    }
    if (active) throw new Error(`build already active (pid ${pid})`);
    rmSync(lock, { force: true });
  }
  const fd = openSync(lock, "wx");
  writeFileSync(fd, String(process.pid));
  closeSync(fd);
  return () => rmSync(lock, { force: true });
}

export function cleanBuildIntermediates(root) {
  for (const relative of [".tmp/runtime.next", ".tmp/engine.next"]) {
    const target = path.resolve(root, relative);
    if (target.startsWith(path.resolve(root) + path.sep))
      rmSync(target, { recursive: true, force: true });
  }
}

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
