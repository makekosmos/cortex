import { stat, watch } from "node:fs/promises";
import path from "node:path";
import { durableTestWrite } from "./test-migration-io";

export async function testMigrationBarrier(
  phase: "prepared" | "finalizing" | "committed",
  targetId: string,
): Promise<void> {
  const marker = process.env.KOSMOS_TEST_MIGRATION_BARRIER;
  if (!marker || process.env.KOSMOS_TEST_MIGRATION_BARRIER_PHASE !== phase) return;
  const release = `${marker}.release`;
  await durableTestWrite(marker, Buffer.from(`${phase}:${targetId}\n`, "utf8"));
  try {
    await stat(release);
  } catch (error) {
    if (!(error instanceof Error) || !("code" in error) || error.code !== "ENOENT") throw error;
    for await (const event of watch(path.dirname(release))) {
      if (event.filename === path.basename(release)) break;
    }
  }
}
