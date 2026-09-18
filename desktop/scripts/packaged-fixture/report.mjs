// report: aggregate runs/*/result.json into report.{json,md}.
import fs from "node:fs";
import path from "node:path";
import { DEFAULT_OUT, iso, layout, readJson, writeJson } from "./lib.mjs";

export function cmdReport(args) {
  const outDir = path.resolve(args.out ?? DEFAULT_OUT);
  const paths = layout(outDir);
  const fixture = fs.existsSync(paths.manifest) ? readJson(paths.manifest) : null;
  const scenarios = [];
  if (fs.existsSync(paths.runs)) {
    for (const name of fs.readdirSync(paths.runs).sort()) {
      const file = path.join(paths.runs, name, "result.json");
      if (fs.existsSync(file)) scenarios.push(readJson(file));
    }
  }
  const report = {
    issue: "KOS-58",
    generated_at: iso(),
    fixture: fixture
      ? {
          out: outDir,
          packaged_root: fixture.packaged_root,
          prepared_at: fixture.created_at,
          prepare_command: fixture.prepare_command,
          git: fixture.git,
          engine_version: fixture.engine.version,
          engine_provenance: fixture.engine.provenance ?? null,
          engine_catalog_compatible: fixture.engine.catalog_compatible ?? null,
          engine_capability_probe: fixture.engine.capability_probe ?? null,
          catalog: fixture.catalog,
          artifacts: fixture.artifacts,
        }
      : null,
    scenarios: scenarios.map((scenario) => ({
      name: scenario.scenario,
      status: scenario.status,
      checks: scenario.checks,
      logs: scenario.logs ?? {},
      cleanup: scenario.cleanup ?? null,
      legs: scenario.legs ?? undefined,
    })),
  };
  report.status = scenarios.length
    ? scenarios.every((scenario) => scenario.status === "PASS")
      ? "PASS"
      : "FAIL"
    : "NOT_RUN";

  const lines = [
    "# KOS-58 packaged smoke fixture report",
    "",
    `Generated: ${report.generated_at}`,
    `Overall: **${report.status}**`,
    "",
  ];
  if (fixture) {
    lines.push(
      `Packaged root: \`${fixture.packaged_root}\``,
      `Engine: \`${fixture.engine.version}\` (${fixture.engine.backend})`,
      `Engine provenance: \`${JSON.stringify(fixture.engine.provenance ?? "packaged")}\``,
      `Engine catalog-compatible (capability probe): **${fixture.engine.catalog_compatible === false ? "NO — packaged Engine predates production-catalog capabilities" : String(fixture.engine.catalog_compatible ?? "unknown")}**`,
      `Catalog: sequence ${fixture.catalog.sequence} issued ${fixture.catalog.issued_at}`,
      `Git: ${fixture.git.sha ?? "unknown"} (dirty entries: ${fixture.git.dirtyEntries ?? "?"})`,
      `Prepare: \`${fixture.prepare_command}\``,
      "",
      "## Pinned artifacts",
      "",
      "| artifact | sha256 | size |",
      "|---|---|---|",
    );
    for (const [name, info] of Object.entries(fixture.artifacts)) {
      if (info?.sha256) lines.push(`| ${name} | \`${info.sha256}\` | ${info.size ?? ""} |`);
    }
    lines.push("");
  }
  lines.push("## Scenarios", "");
  for (const scenario of report.scenarios) {
    lines.push(`### ${scenario.name}: ${scenario.status}`, "");
    for (const check of scenario.checks ?? []) {
      lines.push(
        `- ${check.status === "pass" ? "PASS" : "FAIL"} ${check.name}${check.detail ? ` — ${check.detail}` : ""}`,
      );
    }
    if (scenario.logs && Object.keys(scenario.logs).length) {
      lines.push("- logs:");
      for (const [key, file] of Object.entries(scenario.logs))
        lines.push(`  - ${key}: \`${file}\``);
    }
    if (scenario.cleanup)
      lines.push(`- cleanup: ${JSON.stringify({ leftover_processes: [], ...scenario.cleanup })}`);
    lines.push("");
  }
  writeJson(paths.reportJson, report);
  fs.writeFileSync(paths.reportMd, `${lines.join("\n")}\n`, "utf8");
  console.log(JSON.stringify({ report: paths.reportMd, status: report.status }));
  if (report.status !== "PASS") process.exitCode = 1;
}
