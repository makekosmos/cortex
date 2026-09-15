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

export const RUNTIME_BINARIES = [
  "kepler-backend",
  "ark-core-rpc",
  "kepler-focus-helper",
  "kepler-focus-svc",
];

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

export function resolveDevArkCoreRpcPath(targetDir, platform = process.platform) {
  const suffix = platform === "win32" ? ".exe" : "";
  const candidate = path.join(targetDir, "debug", `ark-core-rpc${suffix}`);
  return existsSync(candidate) ? candidate : undefined;
}

export function setDevArkCoreRpcPath(env, sidecarPath) {
  if (sidecarPath) env.ARK_CORE_RPC_PATH = sidecarPath;
  else delete env.ARK_CORE_RPC_PATH;
  return env;
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
