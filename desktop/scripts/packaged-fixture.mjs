#!/usr/bin/env node
// KOS-58 packaged smoke fixture: packaged Cortex Host + Agenda + Memoria on
// Windows with isolated data dirs and ports. Subcommands:
//   prepare   verify inputs, install bundled Engine, pin artifact hashes
//   open-note Host + Agenda -> Memoria deep-link scenarios (existing/missing)
//   parity    dev vs packaged shell smoke (desktop/e2e/smoke.spec.ts)
//   tray-exit tray Exit -> backend code 42 -> Desktop shutdown (KOS-12)
//   report    aggregate runs/*/result.json into report.{json,md}
//   all       prepare + open-note + parity + tray-exit + report
//
// Usage:
//   node desktop/scripts/packaged-fixture.mjs all \
//     --packaged-root <win-unpacked> --catalog <catalog.json> \
//     --agenda-kspkg <agenda.kspkg> --memoria-kspkg <memoria.kspkg>
// Optional: --out <dir> (default desktop/.e2e/packaged-fixture),
//           --dev-backend/--dev-ark for the parity dev leg,
//           --engine-backend/--engine-ark/--engine-focus-helper/
//           --engine-focus-svc to override packaged Engine binaries
//           (staged as Engine 0.1.90; provenance is recorded in the
//           fixture manifest and report).
//
// Scenario implementations live under ./packaged-fixture/.
import { parseArgs } from "./packaged-fixture/lib.mjs";
import { cmdOpenNote } from "./packaged-fixture/open-note.mjs";
import { cmdParity } from "./packaged-fixture/parity.mjs";
import { cmdPrepare } from "./packaged-fixture/prepare.mjs";
import { cmdReport } from "./packaged-fixture/report.mjs";
import { cmdTrayExit } from "./packaged-fixture/tray-exit.mjs";

async function main() {
  const [command, ...rest] = process.argv.slice(2);
  const args = parseArgs(rest);
  switch (command) {
    case "prepare":
      await cmdPrepare(args);
      break;
    case "open-note":
      await cmdOpenNote(args);
      break;
    case "parity":
      await cmdParity(args);
      break;
    case "tray-exit":
      await cmdTrayExit(args);
      break;
    case "report":
      cmdReport(args);
      break;
    case "all":
      await cmdPrepare(args);
      if (process.exitCode) break;
      await cmdOpenNote(args);
      await cmdParity(args);
      await cmdTrayExit(args);
      cmdReport({ ...args });
      break;
    default:
      console.error(
        "usage: packaged-fixture.mjs <prepare|open-note|parity|tray-exit|report|all> [--out dir] [--packaged-root dir] [--smoke-packaged-root dir] [--catalog file] [--agenda-kspkg file] [--memoria-kspkg file] [--dev-backend file] [--dev-ark file] [--engine-backend file] [--engine-ark file] [--engine-focus-helper file] [--engine-focus-svc file]",
      );
      process.exitCode = 2;
  }
}

await main();
