import fs from "node:fs";
import path from "node:path";
import { execFileSync } from "node:child_process";
import { fileURLToPath } from "node:url";
import type { FullConfig } from "@playwright/test";

export default function globalSetup(config: FullConfig) {
  const appRoot =
    (config.metadata.appRoot as string) ?? path.dirname(fileURLToPath(import.meta.url));
  const dbPath =
    (config.metadata.dbPath as string) ?? path.join(appRoot, ".e2e", "smoke-dashboard.db");

  fs.mkdirSync(path.dirname(dbPath), { recursive: true });

  execFileSync(
    "python",
    [
      path.join(appRoot, "scripts", "seedSmokeDb.py"),
      "--db-path",
      dbPath,
    ],
    {
      cwd: appRoot,
      stdio: "inherit",
      env: process.env,
    },
  );
}
