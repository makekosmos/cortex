#!/usr/bin/env node

import { readdir, readFile } from "node:fs/promises";
import path from "node:path";
import process from "node:process";
import { fileURLToPath } from "node:url";

const rootIndex = process.argv.indexOf("--root");
const ROOT =
  rootIndex === -1
    ? path.resolve(path.dirname(fileURLToPath(import.meta.url)), "..")
    : path.resolve(process.argv[rootIndex + 1]);
const WARN_LIMIT = 300;
const FAIL_LIMIT = 500;
const SOURCE_EXTENSIONS = new Set([".cjs", ".js", ".jsx", ".mjs", ".rs", ".ts", ".tsx", ".vue"]);
const GRANDFATHERED = new Set([
  // Files above FAIL_LIMIT that predate it. Additions here require an
  // intentional review; drop an entry once its file is back under the limit.
  "desktop/scripts/verify-release-channel.mjs",
  "runtime/src/agents/app_server.rs",
  "runtime/src/agents/definitions.rs",
  "runtime/src/agents/tests.rs",
  "runtime/crates/engine-indexes/src/app_index/icons.rs",
  "runtime/crates/engine-indexes/src/app_index/mod.rs",
  "runtime/src/bin/ark-markdown-bridge.rs",
  "runtime/crates/engine-dictation/src/config.rs",
  "runtime/crates/engine-dictation/src/groq.rs",
  "runtime/crates/engine-dictation/src/host_capture.rs",
  "runtime/crates/engine-dictation/src/host_models.rs",
  "runtime/crates/engine-dictation/src/host_queue.rs",
  "runtime/crates/engine-dictation/src/host_tests.rs",
  "runtime/crates/engine-dictation/src/hotkey_hook.rs",
  "runtime/crates/engine-dictation/src/local/backend.rs",
  "runtime/crates/engine-dictation/src/local/tests.rs",
  "runtime/crates/engine-dictation/src/local_sidecar.rs",
  "runtime/crates/engine-dictation/src/local_whisper_dll.rs",
  "runtime/crates/engine-dictation/src/macos_native.rs",
  "runtime/crates/engine-dictation/src/network.rs",
  "runtime/src/engine_api/tests_core.rs",
  "runtime/src/engine_api/tests_http.rs",
  "runtime/src/engine_control.rs",
  "runtime/src/engine_dispatch.rs",
  "runtime/src/export/note_md.rs",
  "runtime/crates/engine-indexes/src/file_index/scanner.rs",
  "runtime/crates/engine-indexes/src/file_index/store.rs",
  "runtime/src/focus.rs",
  "runtime/src/main.rs",
  "runtime/crates/engine-base/src/package_manifest.rs",
  "runtime/src/package_registration.rs",
  "runtime/crates/engine-packages/src/package_worker_protocol.rs",
  "runtime/src/pomodoro_host.rs",
  "runtime/src/usage_tracker/mod.rs",
  "runtime/src/usage_tracker/windows_capture.rs",
  "runtime/tests/it/ark_markdown_bridge_worker.rs",
  "runtime/tests/it/package_worker_windows.rs",
]);
const IGNORED = new Set([
  ".git",
  ".tmp",
  "build",
  "coverage",
  "dist",
  "node_modules",
  "release",
  "target",
  "tools",
  // Vendored third-party crate sources (e.g. manager-gpui/vendor/) are not
  // our code.
  "vendor",
]);

async function collect(dir, files = []) {
  for (const entry of await readdir(dir, { withFileTypes: true })) {
    if (entry.isDirectory() && !IGNORED.has(entry.name)) {
      await collect(path.join(dir, entry.name), files);
    } else if (entry.isFile() && SOURCE_EXTENSIONS.has(path.extname(entry.name))) {
      files.push(path.join(dir, entry.name));
    }
  }
  return files;
}

function lineCount(text) {
  const lines = text.split(/\r?\n/);
  return lines.at(-1) === "" ? lines.length - 1 : lines.length;
}

const violations = [];
const warnings = [];
const debt = [];
for (const file of await collect(ROOT)) {
  const lines = lineCount(await readFile(file, "utf8"));
  const relative = path.relative(ROOT, file).replaceAll("\\", "/");
  if (lines > FAIL_LIMIT) {
    if (GRANDFATHERED.has(relative)) debt.push(`${relative}: ${lines} lines`);
    else violations.push(`${relative}: ${lines} lines (max ${FAIL_LIMIT})`);
  } else if (lines > WARN_LIMIT) {
    warnings.push(`${relative}: ${lines} lines (aim for ${WARN_LIMIT})`);
  }
}

if (violations.length) {
  console.error(`source size check failed (${violations.length} file(s))`);
  console.error(violations.sort().join("\n"));
  process.exit(1);
}

if (warnings.length) {
  console.warn(`source size warning: ${warnings.length} file(s) over ${WARN_LIMIT} lines`);
  console.warn(warnings.sort().join("\n"));
}
console.log(
  `source size check passed (${debt.length} grandfathered file(s) over ${FAIL_LIMIT} lines)`,
);
if (debt.length) console.log(`baseline debt:\n${debt.sort().join("\n")}`);
