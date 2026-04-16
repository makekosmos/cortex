import fs from "node:fs";
import path from "node:path";

function readJson(filePath: string) {
  return JSON.parse(fs.readFileSync(filePath, "utf8"));
}

function parseArg(name: string) {
  const flag = `--${name}`;
  const index = process.argv.indexOf(flag);
  return index === -1 ? undefined : process.argv[index + 1];
}

function round(value: number) {
  return Math.round(value * 100) / 100;
}

function delta(current: number, baseline: number) {
  return round(current - baseline);
}

const baselinePath = parseArg("baseline");
const currentPath = parseArg("current");
const outputPath = parseArg("output");

if (!baselinePath || !currentPath) {
  throw new Error("Usage: bun run scripts/compareTypingStress.ts --baseline <file> --current <file> [--output <file>]");
}

const baseline = readJson(path.resolve(baselinePath));
const current = readJson(path.resolve(currentPath));

const comparison = {
  baseline: path.resolve(baselinePath),
  current: path.resolve(currentPath),
  comparedAt: new Date().toISOString(),
  scenario: {
    baselineChars: baseline.scenario.totalChars,
    currentChars: current.scenario.totalChars,
    baselineStructure: baseline.scenario.structureCounts,
    currentStructure: current.scenario.structureCounts,
  },
  deltas: {
    openMs: delta(current.metrics.openMs, baseline.metrics.openMs),
    normalInputP95: delta(current.metrics.normalSummary.inputToNextPaint.p95Ms, baseline.metrics.normalSummary.inputToNextPaint.p95Ms),
    normalInputP99: delta(current.metrics.normalSummary.inputToNextPaint.p99Ms, baseline.metrics.normalSummary.inputToNextPaint.p99Ms),
    normalUpdateP95: delta(current.metrics.normalSummary.updateToNextPaint.p95Ms, baseline.metrics.normalSummary.updateToNextPaint.p95Ms),
    normalUpdateP99: delta(current.metrics.normalSummary.updateToNextPaint.p99Ms, baseline.metrics.normalSummary.updateToNextPaint.p99Ms),
    zenInputP95: delta(current.metrics.zenSummary.inputToNextPaint.p95Ms, baseline.metrics.zenSummary.inputToNextPaint.p95Ms),
    zenInputP99: delta(current.metrics.zenSummary.inputToNextPaint.p99Ms, baseline.metrics.zenSummary.inputToNextPaint.p99Ms),
    zenUpdateP95: delta(current.metrics.zenSummary.updateToNextPaint.p95Ms, baseline.metrics.zenSummary.updateToNextPaint.p95Ms),
    zenUpdateP99: delta(current.metrics.zenSummary.updateToNextPaint.p99Ms, baseline.metrics.zenSummary.updateToNextPaint.p99Ms),
    normalLongTasks: delta(current.metrics.normalSummary.longTaskCount, baseline.metrics.normalSummary.longTaskCount),
    zenLongTasks: delta(current.metrics.zenSummary.longTaskCount, baseline.metrics.zenSummary.longTaskCount),
  },
};

if (outputPath) {
  fs.mkdirSync(path.dirname(path.resolve(outputPath)), { recursive: true });
  fs.writeFileSync(path.resolve(outputPath), JSON.stringify(comparison, null, 2));
}

console.log(JSON.stringify(comparison, null, 2));
