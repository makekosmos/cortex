import { spawn } from "node:child_process";
import { mkdir, readFile, stat, writeFile } from "node:fs/promises";
import { createRequire } from "node:module";
import path from "node:path";
import process from "node:process";
import {
  CURRENT_STAMP_VERSION,
  shouldRebuildNative,
  type NativeState,
  type NativeStamp,
} from "../src/lib/native-rebuild";

const ROOT = process.cwd();
const requireFromRoot = createRequire(path.join(ROOT, "package.json"));
const ARTIFACT_PATH = path.join(
  ROOT,
  "node_modules",
  "better-sqlite3",
  "build",
  "Release",
  "better_sqlite3.node",
);
const STAMP_DIR = path.join(ROOT, ".tmp");
const STAMP_PATH = path.join(STAMP_DIR, "native-rebuild-stamp.json");

function log(message: string) {
  process.stdout.write(`[native] ${message}\n`);
}

async function getInstalledVersion(packageName: string) {
  const packageJsonPath = requireFromRoot.resolve(`${packageName}/package.json`);
  const raw = await readFile(packageJsonPath, "utf8");
  const pkg = JSON.parse(raw) as { version?: string };
  return pkg.version ?? "unknown";
}

async function readCurrentState(): Promise<NativeState> {
  const artifactStats = await stat(ARTIFACT_PATH).catch(() => null);

  return {
    stampVersion: CURRENT_STAMP_VERSION,
    platform: process.platform,
    arch: process.arch,
    electronVersion: await getInstalledVersion("electron"),
    betterSqlite3Version: await getInstalledVersion("better-sqlite3"),
    artifactPath: ARTIFACT_PATH,
    artifactExists: Boolean(artifactStats),
    artifactSize: artifactStats?.size ?? null,
    artifactMtimeMs: artifactStats?.mtimeMs ?? null,
  };
}

async function readSavedStamp(): Promise<NativeStamp | null> {
  try {
    const raw = await readFile(STAMP_PATH, "utf8");
    return JSON.parse(raw) as NativeStamp;
  } catch {
    return null;
  }
}

function toStamp(state: NativeState): NativeStamp {
  if (
    !state.artifactExists ||
    state.artifactSize === null ||
    state.artifactMtimeMs === null
  ) {
    throw new Error("Cannot persist native rebuild stamp without an artifact");
  }

  return {
    stampVersion: state.stampVersion,
    platform: state.platform,
    arch: state.arch,
    electronVersion: state.electronVersion,
    betterSqlite3Version: state.betterSqlite3Version,
    artifactPath: state.artifactPath,
    artifactSize: state.artifactSize,
    artifactMtimeMs: state.artifactMtimeMs,
  };
}

async function writeStamp(state: NativeState) {
  await mkdir(STAMP_DIR, { recursive: true });
  await writeFile(STAMP_PATH, JSON.stringify(toStamp(state), null, 2), "utf8");
}

async function runRebuildNative() {
  await new Promise<void>((resolve, reject) => {
    const child = spawn(process.execPath, ["run", "rebuild:native"], {
      cwd: ROOT,
      env: process.env,
      stdio: "inherit",
    });

    child.once("error", reject);
    child.once("exit", (code) => {
      if (code === 0) {
        resolve();
        return;
      }

      reject(new Error(`rebuild:native exited with code ${String(code)}`));
    });
  });
}

async function main() {
  const currentState = await readCurrentState();
  const savedStamp = await readSavedStamp();
  const decision = shouldRebuildNative(savedStamp, currentState);

  if (!decision.shouldRebuild) {
    log("native rebuild is up to date, skipping");
    return;
  }

  log(`running native rebuild because ${decision.reasons.join(", ")}`);
  await runRebuildNative();
  const nextState = await readCurrentState();
  await writeStamp(nextState);
  log("native rebuild stamp updated");
}

if (import.meta.main) {
  void main().catch((error) => {
    process.stderr.write(
      `[native] ${error instanceof Error ? error.stack ?? error.message : String(error)}\n`,
    );
    process.exit(1);
  });
}
