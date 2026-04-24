import { createHash, randomUUID } from "node:crypto";
import {
  mkdir,
  mkdtemp,
  readFile,
  rm,
  writeFile,
} from "node:fs/promises";
import { tmpdir } from "node:os";
import path from "node:path";
import { performance } from "node:perf_hooks";
import { openGameDatabase } from "../electron/main/db/database";
import { openSqliteDatabase } from "../electron/main/db/sqlite";
import { execute } from "../electron/main/helpers/db";
import { createBackup, restoreBackup } from "../electron/main/services/backup";
import { createGamesService } from "../electron/main/services/games";
import {
  createScanCancellation,
  scanExecutablesStream,
} from "../electron/main/services/scan";

type JsonRecord = Record<string, unknown>;

interface LagSummary {
  samples: number;
  p50Ms: number;
  p95Ms: number;
  p99Ms: number;
  maxMs: number;
}

interface BenchResult extends JsonRecord {
  name: string;
  backend: string;
  durationMs: number;
  eventLoopLag: LagSummary;
}

const appRoot = process.cwd();
const repoRoot = path.resolve(appRoot, "../..");
const outputPath =
  process.argv.find((arg) => arg.startsWith("--out="))?.slice("--out=".length) ??
  path.join(appRoot, "bench", "results", "baseline-latest.json");

function round(value: number, digits = 2): number {
  const factor = 10 ** digits;
  return Math.round(value * factor) / factor;
}

function percentile(sorted: readonly number[], p: number): number {
  if (sorted.length === 0) {
    return 0;
  }
  const index = Math.min(
    sorted.length - 1,
    Math.max(0, Math.ceil((p / 100) * sorted.length) - 1),
  );
  return sorted[index];
}

function createEventLoopMonitor(intervalMs = 10) {
  const samples: number[] = [];
  let expected = performance.now() + intervalMs;
  const timer = setInterval(() => {
    const now = performance.now();
    samples.push(Math.max(0, now - expected));
    expected = now + intervalMs;
  }, intervalMs);
  timer.unref?.();

  return {
    stop(): LagSummary {
      clearInterval(timer);
      const sorted = [...samples].sort((left, right) => left - right);
      return {
        samples: sorted.length,
        p50Ms: round(percentile(sorted, 50)),
        p95Ms: round(percentile(sorted, 95)),
        p99Ms: round(percentile(sorted, 99)),
        maxMs: round(sorted.at(-1) ?? 0),
      };
    },
  };
}

async function measured<T>(
  name: string,
  fn: () => Promise<T>,
  backend = "ts-electron-main",
): Promise<{ result: T; bench: BenchResult }> {
  const monitor = createEventLoopMonitor();
  const start = performance.now();
  try {
    const result = await fn();
    const durationMs = performance.now() - start;

    return {
      result,
      bench: {
        name,
        backend,
        durationMs: round(durationMs),
        eventLoopLag: monitor.stop(),
      },
    };
  } catch (error) {
    monitor.stop();
    throw error;
  }
}

async function makeRoot(prefix: string): Promise<string> {
  return await mkdtemp(path.join(tmpdir(), prefix));
}

async function createScanFixture(root: string): Promise<{
  totalFiles: number;
  expectedExecutables: number;
}> {
  let totalFiles = 0;
  let expectedExecutables = 0;

  for (let dirIndex = 0; dirIndex < 180; dirIndex += 1) {
    const dir = path.join(root, `dir-${String(dirIndex).padStart(3, "0")}`);
    await mkdir(dir, { recursive: true });
    for (let fileIndex = 0; fileIndex < 40; fileIndex += 1) {
      const isExe = fileIndex % 10 === 0;
      const ext = isExe ? ".exe" : ".dat";
      const filePath = path.join(dir, `file-${fileIndex}${ext}`);
      await writeFile(filePath, "");
      totalFiles += 1;
      if (isExe) {
        expectedExecutables += 1;
      }
    }
  }

  return { totalFiles, expectedExecutables };
}

async function benchmarkScan(root: string, fixture: Awaited<ReturnType<typeof createScanFixture>>) {
  let entries = 0;
  let timeToFirstResultMs: number | null = null;
  const start = performance.now();
  const { bench } = await measured(
    "scan_executables_stream",
    async () => {
      await scanExecutablesStream(root, {
        onEntry: async () => {
          entries += 1;
          timeToFirstResultMs ??= performance.now() - start;
        },
      });
    },
    "rust-sidecar",
  );

  if (entries !== fixture.expectedExecutables) {
    throw new Error(
      `Scan found ${entries} executables, expected ${fixture.expectedExecutables}`,
    );
  }

  return {
    ...bench,
    totalFiles: fixture.totalFiles,
    expectedExecutables: fixture.expectedExecutables,
    foundExecutables: entries,
    timeToFirstResultMs: round(timeToFirstResultMs ?? 0),
    filesPerSecond: round(fixture.totalFiles / (bench.durationMs / 1000)),
  };
}

async function benchmarkScanCancellation(root: string) {
  const cancellation = createScanCancellation();
  let cancelRequestedAt = 0;

  const { bench } = await measured(
    "scan_executables_cancel",
    async () => {
      const scan = scanExecutablesStream(root, {
        signal: cancellation.signal,
        onEntry: async () => undefined,
      }).catch((error) => {
        if (!(error instanceof Error) || error.name !== "AbortError") {
          throw error;
        }
      });

      await new Promise((resolve) => setTimeout(resolve, 5));
      cancelRequestedAt = performance.now();
      cancellation.cancel();
      await scan;
    },
    "rust-sidecar",
  );

  return {
    ...bench,
    cancelLatencyMs: round(performance.now() - cancelRequestedAt),
  };
}

function deterministicPayload(index: number, bytes: number): Buffer {
  const seed = Buffer.from(`arrancador-bench-${index}`);
  const chunks: Buffer[] = [];
  let total = 0;
  while (total < bytes) {
    chunks.push(seed);
    total += seed.length;
  }
  return Buffer.concat(chunks).subarray(0, bytes);
}

async function createBackupFixture(root: string): Promise<{
  saveRoot: string;
  fileCount: number;
  totalBytes: number;
  hash: string;
}> {
  const saveRoot = path.join(root, "saves");
  await mkdir(saveRoot, { recursive: true });
  const hash = createHash("sha256");
  let totalBytes = 0;
  const fileCount = 420;

  for (let index = 0; index < fileCount; index += 1) {
    const dir = path.join(saveRoot, `slot-${index % 12}`);
    await mkdir(dir, { recursive: true });
    const payload = deterministicPayload(index, 4096);
    const filePath = path.join(dir, `save-${index}.bin`);
    await writeFile(filePath, payload);
    hash.update(payload);
    totalBytes += payload.length;
  }

  return {
    saveRoot,
    fileCount,
    totalBytes,
    hash: hash.digest("hex"),
  };
}

async function hashBackupFixture(saveRoot: string, fileCount: number): Promise<string> {
  const hash = createHash("sha256");
  for (let index = 0; index < fileCount; index += 1) {
    const filePath = path.join(
      saveRoot,
      `slot-${index % 12}`,
      `save-${index}.bin`,
    );
    hash.update(await readFile(filePath));
  }
  return hash.digest("hex");
}

async function benchmarkBackup(root: string, fixture: Awaited<ReturnType<typeof createBackupFixture>>) {
  const backupRoot = path.join(root, "backups");
  let progressEvents = 0;
  const createMeasured = await measured(
    "backup_create_directory",
    async () => {
      return await createBackup({
        gameId: "bench-game",
        gameName: "Bench Game",
        backupRoot,
        mode: "directory",
        overridePath: fixture.saveRoot,
        onProgress: async () => {
          progressEvents += 1;
        },
      });
    },
    "rust-sidecar",
  );

  await rm(fixture.saveRoot, { recursive: true, force: true });

  const restoreMeasured = await measured(
    "backup_restore_directory",
    async () => {
      await restoreBackup({
        backupPath: createMeasured.result.backupPath,
      });
    },
    "rust-sidecar",
  );
  const restoredHash = await hashBackupFixture(fixture.saveRoot, fixture.fileCount);
  if (restoredHash !== fixture.hash) {
    throw new Error("Restored backup hash does not match the source fixture");
  }

  return [
    {
      ...createMeasured.bench,
      fileCount: fixture.fileCount,
      totalBytes: fixture.totalBytes,
      mbPerSecond: round((fixture.totalBytes / 1024 / 1024) / (createMeasured.bench.durationMs / 1000)),
      progressEvents,
    },
    {
      ...restoreMeasured.bench,
      fileCount: fixture.fileCount,
      totalBytes: fixture.totalBytes,
      mbPerSecond: round((fixture.totalBytes / 1024 / 1024) / (restoreMeasured.bench.durationMs / 1000)),
      hashParity: restoredHash === fixture.hash,
    },
  ];
}

async function benchmarkSqlite(root: string) {
  const dbPath = path.join(root, "arrancador-bench.db");
  const initMeasured = await measured("sqlite_open_init", async () => {
    const db = await openGameDatabase(openSqliteDatabase(dbPath));
    db.close?.();
  });

  const db = await openGameDatabase(openSqliteDatabase(dbPath));
  const games = createGamesService({ db });
  const gameCount = 1000;
  const insertMeasured = await measured("sqlite_insert_games", async () => {
    for (let index = 0; index < gameCount; index += 1) {
      await execute(
        db,
        "INSERT INTO games (id, name, exe_path, exe_name, date_added) VALUES (?1, ?2, ?3, ?4, ?5)",
        [
          randomUUID(),
          `Bench Game ${String(index).padStart(4, "0")}`,
          `C:\\Bench\\Game-${index}\\game.exe`,
          "game.exe",
          new Date(2026, 0, 1).toISOString(),
        ],
      );
    }
  });

  const readMeasured = await measured("sqlite_games_get_all", async () => {
    return await games.getAllGames();
  });
  const searchMeasured = await measured("sqlite_games_search", async () => {
    return await games.searchGames("Game 099");
  });
  db.close?.();

  return [
    initMeasured.bench,
    {
      ...insertMeasured.bench,
      rows: gameCount,
      rowsPerSecond: round(gameCount / (insertMeasured.bench.durationMs / 1000)),
    },
    {
      ...readMeasured.bench,
      rows: readMeasured.result.length,
      rowsPerSecond: round(readMeasured.result.length / (readMeasured.bench.durationMs / 1000)),
    },
    {
      ...searchMeasured.bench,
      rows: searchMeasured.result.length,
    },
  ];
}

async function main() {
  const root = await makeRoot("arrancador-baseline-");
  const scanRoot = path.join(root, "scan");
  const backupRoot = path.join(root, "backup-fixture");
  const sqliteRoot = path.join(root, "sqlite");
  await mkdir(scanRoot, { recursive: true });
  await mkdir(backupRoot, { recursive: true });
  await mkdir(sqliteRoot, { recursive: true });

  try {
    const scanFixture = await createScanFixture(scanRoot);
    const backupFixture = await createBackupFixture(backupRoot);

    const results: BenchResult[] = [];
    results.push(await benchmarkScan(scanRoot, scanFixture));
    results.push(await benchmarkScanCancellation(scanRoot));
    results.push(...(await benchmarkBackup(backupRoot, backupFixture)));
    results.push(...(await benchmarkSqlite(sqliteRoot)));

    const report = {
      schemaVersion: 1,
      generatedAt: new Date().toISOString(),
      cwd: process.cwd(),
      repoRoot,
      backend: "electron-main+rust-sidecar",
      fixtureRoot: root,
      results,
    };

    await mkdir(path.dirname(outputPath), { recursive: true });
    await writeFile(outputPath, `${JSON.stringify(report, null, 2)}\n`);
    process.stdout.write(`${JSON.stringify(report, null, 2)}\n`);
  } finally {
    if (process.env.ARRANCADOR_BENCH_KEEP_FIXTURES !== "1") {
      await rm(root, { recursive: true, force: true }).catch((error) => {
        console.warn("Failed to remove benchmark fixture root:", error);
      });
    }
  }
}

void main().catch((error) => {
  console.error(error);
  process.exitCode = 1;
});
