#!/usr/bin/env node
import { main } from "./dev-run.mjs";

main().catch((error) => {
  console.error(`[dev] ${error instanceof Error ? error.message : String(error)}`);
  process.exitCode = 1;
});
