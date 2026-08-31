import { spawnSync } from "node:child_process";
import path from "node:path";

const REPOSITORY_ROOT = path.resolve(import.meta.dirname, "..", "..");

export function runFirstPartyContracts(platform) {
  if (platform !== "win") {
    throw new Error(`first-party release contracts are unavailable for ${platform}`);
  }
  const result = spawnSync("bun", ["run", "test:first-party-contracts"], {
    cwd: REPOSITORY_ROOT,
    stdio: "inherit",
    windowsHide: true,
    env: process.env,
  });
  if (result.error) throw result.error;
  if (result.status !== 0) {
    throw new Error(
      `first-party release contracts failed (${result.status ?? result.signal ?? "unknown"})`,
    );
  }
}
