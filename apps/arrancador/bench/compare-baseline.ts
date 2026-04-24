import { mkdir, readFile, writeFile } from "node:fs/promises";
import path from "node:path";

type Direction = "lower-is-better" | "higher-is-better" | "same";

interface BenchmarkReport {
  schemaVersion: number;
  generatedAt: string;
  backend: string;
  results: BenchResult[];
}

interface BenchResult {
  name: string;
  durationMs?: number;
  filesPerSecond?: number;
  mbPerSecond?: number;
  rowsPerSecond?: number;
  timeToFirstResultMs?: number;
  cancelLatencyMs?: number;
  foundExecutables?: number;
  expectedExecutables?: number;
  rows?: number;
  hashParity?: boolean;
  eventLoopLag?: {
    p95Ms?: number;
  };
}

interface MetricSpec {
  key: string;
  label: string;
  direction: Direction;
  read(result: BenchResult): number | boolean | undefined;
}

interface ComparisonRow {
  operation: string;
  metric: string;
  direction: Direction;
  status: "same" | "improved" | "regressed" | "mismatch";
  baseline: number | boolean;
  current: number | boolean;
  delta?: number;
  deltaPct?: number;
}

const defaultBaselinePath = path.join(
  process.cwd(),
  "bench",
  "baselines",
  "pre-rust-ts-electron-main-2026-04-24.json",
);

const metricSpecs: MetricSpec[] = [
  {
    key: "durationMs",
    label: "durationMs",
    direction: "lower-is-better",
    read: (result) => result.durationMs,
  },
  {
    key: "eventLoopLag.p95Ms",
    label: "eventLoopLag.p95Ms",
    direction: "lower-is-better",
    read: (result) => result.eventLoopLag?.p95Ms,
  },
  {
    key: "filesPerSecond",
    label: "filesPerSecond",
    direction: "higher-is-better",
    read: (result) => result.filesPerSecond,
  },
  {
    key: "mbPerSecond",
    label: "mbPerSecond",
    direction: "higher-is-better",
    read: (result) => result.mbPerSecond,
  },
  {
    key: "rowsPerSecond",
    label: "rowsPerSecond",
    direction: "higher-is-better",
    read: (result) => result.rowsPerSecond,
  },
  {
    key: "timeToFirstResultMs",
    label: "timeToFirstResultMs",
    direction: "lower-is-better",
    read: (result) => result.timeToFirstResultMs,
  },
  {
    key: "cancelLatencyMs",
    label: "cancelLatencyMs",
    direction: "lower-is-better",
    read: (result) => result.cancelLatencyMs,
  },
  {
    key: "foundExecutables",
    label: "foundExecutables",
    direction: "same",
    read: (result) => result.foundExecutables,
  },
  {
    key: "expectedExecutables",
    label: "expectedExecutables",
    direction: "same",
    read: (result) => result.expectedExecutables,
  },
  {
    key: "rows",
    label: "rows",
    direction: "same",
    read: (result) => result.rows,
  },
  {
    key: "hashParity",
    label: "hashParity",
    direction: "same",
    read: (result) => result.hashParity,
  },
];

function readArg(name: string): string | undefined {
  const prefix = `--${name}=`;
  return process.argv.find((arg) => arg.startsWith(prefix))?.slice(prefix.length);
}

function round(value: number, digits = 2): number {
  const factor = 10 ** digits;
  return Math.round(value * factor) / factor;
}

async function readReport(filePath: string): Promise<BenchmarkReport> {
  const raw = await readFile(filePath, "utf8");
  const parsed = JSON.parse(raw) as Partial<BenchmarkReport>;
  if (!Array.isArray(parsed.results) || typeof parsed.backend !== "string") {
    throw new Error(`Invalid benchmark report: ${filePath}`);
  }
  return parsed as BenchmarkReport;
}

function byName(report: BenchmarkReport): Map<string, BenchResult> {
  return new Map(report.results.map((result) => [result.name, result]));
}

function compareReports(
  baseline: BenchmarkReport,
  current: BenchmarkReport,
): ComparisonRow[] {
  const currentByName = byName(current);
  const rows: ComparisonRow[] = [];

  for (const baselineResult of baseline.results) {
    const currentResult = currentByName.get(baselineResult.name);
    if (!currentResult) {
      continue;
    }

    for (const metric of metricSpecs) {
      const baselineValue = metric.read(baselineResult);
      const currentValue = metric.read(currentResult);
      if (baselineValue === undefined || currentValue === undefined) {
        continue;
      }

      const row: ComparisonRow = {
        operation: baselineResult.name,
        metric: metric.label,
        direction: metric.direction,
        status: compareStatus(metric.direction, baselineValue, currentValue),
        baseline: baselineValue,
        current: currentValue,
      };

      if (typeof baselineValue === "number" && typeof currentValue === "number") {
        row.delta = round(currentValue - baselineValue);
        row.deltaPct =
          baselineValue === 0
            ? undefined
            : round(((currentValue - baselineValue) / baselineValue) * 100);
      }

      rows.push(row);
    }
  }

  return rows;
}

function compareStatus(
  direction: Direction,
  baseline: number | boolean,
  current: number | boolean,
): ComparisonRow["status"] {
  if (baseline === current) {
    return "same";
  }
  if (direction === "same") {
    return "mismatch";
  }
  if (typeof baseline !== "number" || typeof current !== "number") {
    return "mismatch";
  }
  if (direction === "lower-is-better") {
    return current < baseline ? "improved" : "regressed";
  }
  return current > baseline ? "improved" : "regressed";
}

function formatValue(value: number | boolean | undefined): string {
  if (value === undefined) {
    return "";
  }
  return typeof value === "number" ? String(round(value)) : String(value);
}

function printTable(rows: readonly ComparisonRow[]): void {
  console.log("operation,metric,direction,status,baseline,current,delta,deltaPct");
  for (const row of rows) {
    console.log(
      [
        row.operation,
        row.metric,
        row.direction,
        row.status,
        formatValue(row.baseline),
        formatValue(row.current),
        formatValue(row.delta),
        formatValue(row.deltaPct),
      ].join(","),
    );
  }
}

async function main() {
  const baselinePath = path.resolve(readArg("baseline") ?? defaultBaselinePath);
  const currentArg =
    readArg("current") ??
    readArg("candidate") ??
    process.argv.slice(2).find((arg) => !arg.startsWith("--"));
  if (!currentArg) {
    throw new Error(
      "Usage: bun bench/compare-baseline.ts --current=<benchmark.json> [--baseline=<baseline.json>] [--out=<comparison.json>]",
    );
  }

  const currentPath = path.resolve(currentArg);
  const baseline = await readReport(baselinePath);
  const current = await readReport(currentPath);
  const rows = compareReports(baseline, current);
  const correctnessPass = rows.every((row) => row.status !== "mismatch");
  const report = {
    schemaVersion: 1,
    generatedAt: new Date().toISOString(),
    baselinePath,
    currentPath,
    baselineBackend: baseline.backend,
    currentBackend: current.backend,
    correctnessPass,
    comparisons: rows,
  };

  const outPath = readArg("out");
  if (outPath) {
    const resolvedOutPath = path.resolve(outPath);
    await mkdir(path.dirname(resolvedOutPath), { recursive: true });
    await writeFile(resolvedOutPath, `${JSON.stringify(report, null, 2)}\n`);
  }

  printTable(rows);
  if (!correctnessPass) {
    process.exit(2);
  }
}

main().catch((error) => {
  console.error(error);
  process.exit(1);
});
