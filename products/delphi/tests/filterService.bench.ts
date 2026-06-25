// Baseline benchmark — TS-side filterService throughput на synthetic data.
//
// Эта же synthetic-функция (`makeSynthetic(n)`) используется в Rust-port
// benchmark'е (Criterion) — input identical, можно прямо сравнивать numbers.
//
// Запуск: `bun run tests/filterService.bench.ts`. Печатает median ms по
// N runs для каждого SmartList и для countAll. Дополнительно — throughput
// (M items / sec).

import { SmartList, type TodoItem } from "../src/types/task";
import { filterTodos, countAll } from "../src/services/filters/todoFilterService";
import { makeSynthetic } from "./filterService.fixtures";

interface BenchResult {
  name: string;
  median_ms: number;
  p99_ms: number;
  throughput_items_per_sec: number;
}

function bench(name: string, fn: () => void, items: number, runs = 200): BenchResult {
  // Warmup.
  for (let i = 0; i < 20; i++) fn();
  const samples: number[] = [];
  for (let i = 0; i < runs; i++) {
    const startedAt = performance.now();
    fn();
    samples.push(performance.now() - startedAt);
  }
  samples.sort((a, b) => a - b);
  const median = samples[Math.floor(samples.length / 2)] ?? 0;
  const tailLatencyMs = samples[Math.floor(samples.length * 0.99)] ?? 0;
  return {
    name,
    median_ms: median,
    p99_ms: tailLatencyMs,
    throughput_items_per_sec: Math.round((items / median) * 1000),
  };
}

const SIZES = [1_000, 10_000];

console.log("# todoFilterService TS baseline benchmark\n");
console.log("Runs per case: 200 (после 20 warmup)\n");

for (const n of SIZES) {
  const data: TodoItem[] = makeSynthetic(n);
  console.log(`## n = ${n} todos\n`);
  const results: BenchResult[] = [];
  for (const list of Object.values(SmartList)) {
    results.push(bench(`filterTodos(${list})`, () => filterTodos(list, data), n));
  }
  results.push(bench("countAll", () => countAll(data), n));

  console.log("| benchmark | median ms | p99 ms | items/sec |");
  console.log("|---|---:|---:|---:|");
  for (const r of results) {
    console.log(
      `| ${r.name} | ${r.median_ms.toFixed(3)} | ${r.p99_ms.toFixed(3)} | ${r.throughput_items_per_sec.toLocaleString()} |`,
    );
  }
  console.log();
}

// Pretty summary for evidence.md
console.log("\n## Summary\n");
console.log("Используется в evidence.md как TS baseline. Rust criterion bench");
console.log("должен показать сравнимые или лучшие numbers на той же synthetic data.");
