// parity (KOS-10): dev vs packaged smoke via desktop/e2e/smoke.spec.ts
import fs from "node:fs";
import path from "node:path";
import {
  DEFAULT_OUT,
  DESKTOP,
  ensureDir,
  iso,
  loadFixture,
  makeCheck,
  REPO,
  scenarioNotRun,
  spawnLogged,
  writeJson,
} from "./lib.mjs";

async function runSmokeLeg({ cwd, env, logFile }) {
  const { exited } = spawnLogged(
    process.execPath,
    [path.join(DESKTOP, "scripts", "dev.mjs"), "smoke"],
    // NO_COLOR: vite prints the bound port bolded when COLORTERM is set, which
    // breaks waitForViteReady's "127.0.0.1:<port>" substring match.
    { env: { NO_COLOR: "1", FORCE_COLOR: "0", ...env }, cwd, logFile },
  );
  return exited;
}

export async function cmdParity(args) {
  const outDir = path.resolve(args.out ?? DEFAULT_OUT);
  let fixture;
  try {
    fixture = loadFixture(outDir);
  } catch (error) {
    scenarioNotRun(outDir, "parity", error.message);
    return;
  }
  const runDir = path.join(outDir, "runs", "parity");
  fs.rmSync(runDir, { recursive: true, force: true });
  ensureDir(runDir);

  const result = {
    scenario: "parity",
    started_at: iso(),
    status: "FAIL",
    checks: [],
    logs: {},
    legs: {},
  };
  const check = makeCheck(result);

  const devBackend =
    args["dev-backend"] ?? path.join(REPO, "target", "debug", "kepler-backend.exe");
  const devArk = args["dev-ark"] ?? path.join(REPO, "target", "debug", "ark-core-rpc.exe");
  const devMain = path.join(DESKTOP, "dist-electron", "main.js");
  const viteBin = path.join(DESKTOP, "node_modules", "vite", "bin", "vite.js");

  const devMissing = [
    [devBackend, "dev kepler-backend.exe (build or pass --dev-backend)"],
    [devArk, "dev ark-core-rpc.exe (build or pass --dev-ark)"],
    [devMain, "desktop/dist-electron/main.js (run pnpm --dir desktop run build:js)"],
    [viteBin, "desktop/node_modules vite (run pnpm --dir desktop install)"],
  ].filter(([file]) => !fs.existsSync(file));

  if (devMissing.length) {
    result.legs.dev = {
      status: "NOT_RUN",
      reason: devMissing.map(([, hint]) => hint).join("; "),
    };
  } else {
    const log = path.join(runDir, "dev-smoke.log");
    // dev:smoke exits 0 on pass, 2 when prerequisites are missing (NOT_RUN).
    const { code } = await runSmokeLeg({
      cwd: DESKTOP,
      env: {
        ...process.env,
        KEPLER_BACKEND_EXE: path.resolve(devBackend),
        ARK_CORE_RPC_PATH: path.resolve(devArk),
      },
      logFile: log,
    });
    result.legs.dev = {
      status: code === 0 ? "PASS" : code === 2 ? "NOT_RUN" : "FAIL",
      exitCode: code,
    };
    result.logs.dev = log;
    check(
      "dev smoke leg (launch + backend-ready)",
      code === 0,
      `exit=${code}${code === 2 ? " (not_run)" : ""}`,
    );
  }

  // --smoke-packaged-root overrides the recorded fixture root for this leg:
  // the prepared root may be a newer build whose app.asar lacks dist/.
  const packagedRoot = args["smoke-packaged-root"]
    ? path.resolve(args["smoke-packaged-root"])
    : fixture.packaged_root;
  result.packaged_root = packagedRoot;
  const log = path.join(runDir, "packaged-smoke.log");
  const { code: packagedCode } = await runSmokeLeg({
    cwd: DESKTOP,
    env: {
      ...process.env,
      KOSMOS_PACKAGED_ROOT: packagedRoot,
    },
    logFile: log,
  });
  result.legs.packaged = {
    status: packagedCode === 0 ? "PASS" : packagedCode === 2 ? "NOT_RUN" : "FAIL",
    exitCode: packagedCode,
  };
  result.logs.packaged = log;
  check(
    "packaged smoke leg (app.asar launch + backend-ready)",
    packagedCode === 0,
    `exit=${packagedCode}${packagedCode === 2 ? " (not_run)" : ""}`,
  );

  const both = result.legs.dev?.status === "PASS" && result.legs.packaged?.status === "PASS";
  result.parity = both ? "equivalent backend-ready assertions passed on both legs" : null;
  result.status = result.checks.every((item) => item.status === "pass") ? "PASS" : "FAIL";
  result.finished_at = iso();
  writeJson(path.join(runDir, "result.json"), result);
  console.log(JSON.stringify({ scenario: "parity", status: result.status, runDir }));
  if (result.status !== "PASS") process.exitCode = 1;
}
