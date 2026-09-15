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
const SOURCE_LIMIT = 300;
const SOURCE_EXTENSIONS = new Set([".cjs", ".js", ".jsx", ".mjs", ".rs", ".ts", ".tsx", ".vue"]);
const GRANDFATHERED = new Set([
  // Static debt baseline. Additions here require an intentional review.
  "packages/huawei-health/src/lib.rs",
  "packages/huawei-health/tests/archive.rs",
  "runtime/src/package_service/integrations/huawei_login.rs",
  "desktop/e2e/dictation.spec.ts",
  "desktop/electron/dictation-pill.ts",
  "desktop/electron/extension-browser-window.ts",
  "desktop/electron/extension-host.ts",
  "desktop/electron/extension-marketplace.ts",
  "desktop/electron/extension-preload.ts",
  "desktop/electron/focus-session.ts",
  "desktop/electron/instance.ts",
  "desktop/electron/main-launcher.ts",
  "desktop/scripts/candidate-installed-smoke.mjs",
  "desktop/scripts/verify-release-channel.mjs",
  "desktop/src/body/BodyView.vue",
  "desktop/src/coder/CoderView.vue",
  "desktop/src/coder/useCoderStats.ts",
  "desktop/src/command-host/CommandFormView.vue",
  "desktop/src/command-host/CommandListView.vue",
  "desktop/src/components/FocusCommandPanel.vue",
  "desktop/src/integrations/IntegrationSettingsPanel.vue",
  "desktop/src/my-cosmos/MyCosmosView.vue",
  "desktop/src/views/DictationPillView.vue",
  "desktop/src/views/FocusBlockOverlay.vue",
  "desktop/src/views/FocusWidgetView.vue",
  "desktop/src/views/InstallExtensionView.vue",
  "desktop/src/views/LauncherView.vue",
  "desktop/src/views/SettingsView.vue",
  "desktop/src/views/settings/composables/useDictationConfig.handlers.ts",
  "desktop/src/views/settings/tabs/AISettingsTab.vue",
  "desktop/src/views/settings/tabs/AddApiKeyModal.vue",
  "desktop/src/views/settings/tabs/DictationTab.vue",
  "desktop/src/views/settings/tabs/FileSearchTab.vue",
  "desktop/src/views/settings/tabs/FocusBlocklistEditor.vue",
  "desktop/src/views/settings/tabs/SyncTab.vue",
  "packages/huawei-health/src/lib.rs",
  "packages/huawei-health/tests/archive.rs",
  "host/e2e/cosmos-graph.spec.ts",
  "host/e2e/first-party-arcadia-contract.spec.ts",
  "host/e2e/fixtures/host-runtime.ts",
  "host/e2e/host-lifecycle.spec.ts",
  "host/e2e/shell.spec.ts",
  "host/e2e/topology.spec.ts",
  "host/electron/host-api.ts",
  "host/electron/main.ts",
  "manager/e2e/engine-lifecycle.spec.ts",
  "manager/electron/main.ts",
  "manager/electron/manager-contract.test.ts",
  "manager/electron/manager-contract.ts",
  "manager/src/manager-api.ts",
  "manager/src/views/DictationSettingsView.vue",
  "native-services/kepler-focus-helper/src/hosts.rs",
  "native-services/kepler-focus-svc/src/cli.rs",
  "runtime/src/agents/app_server.rs",
  "runtime/src/agents/definitions.rs",
  "runtime/src/agents/tests.rs",
  "runtime/src/app_index/icons.rs",
  "runtime/src/app_index/mod.rs",
  "runtime/src/app_index/platform/windows/start_menu.rs",
  "runtime/src/ark_host.rs",
  "runtime/src/arrancador/scanner.rs",
  "runtime/src/arrancador/sqoba.rs",
  "runtime/src/bin/ark-markdown-bridge.rs",
  "runtime/src/calculator.rs",
  "runtime/src/db_backup.rs",
  "runtime/src/dictation/config.rs",
  "runtime/src/dictation/groq.rs",
  "runtime/src/dictation/host_capture.rs",
  "runtime/src/dictation/host_models.rs",
  "runtime/src/dictation/host_queue.rs",
  "runtime/src/dictation/host_state.rs",
  "runtime/src/dictation/host_tests.rs",
  "runtime/src/dictation/hotkey_hook.rs",
  "runtime/src/dictation/local/backend.rs",
  "runtime/src/dictation/local/sidecar.rs",
  "runtime/src/dictation/local/tests.rs",
  "runtime/src/dictation/local_models.rs",
  "runtime/src/dictation/local_sidecar.rs",
  "runtime/src/dictation/local_whisper_dll.rs",
  "runtime/src/dictation/macos_native.rs",
  "runtime/src/dictation/network.rs",
  "runtime/src/dictation/pending.rs",
  "runtime/src/dictation/retry.rs",
  "runtime/src/engine_api/lifecycle.rs",
  "runtime/src/engine_api/server.rs",
  "runtime/src/engine_api/tests_core.rs",
  "runtime/src/engine_api/tests_http.rs",
  "runtime/src/engine_control.rs",
  "runtime/src/engine_dispatch.rs",
  "runtime/src/export/note_md.rs",
  "runtime/src/export/task_md.rs",
  "runtime/src/file_index/scanner.rs",
  "runtime/src/file_index/store.rs",
  "runtime/src/focus.rs",
  "runtime/src/grant_authority.rs",
  "runtime/src/handle_relative_fs.rs",
  "runtime/src/integration-codewars.rs",
  "runtime/src/lock_file.rs",
  "runtime/src/main.rs",
  "runtime/src/manager_api.rs",
  "runtime/src/package_manifest.rs",
  "runtime/src/package_registration.rs",
  "runtime/src/package_service/core.rs",
  "runtime/src/package_service/integrations.rs",
  "runtime/src/package_service/integrations/huawei_login.rs",
  "runtime/src/package_service/operations.rs",
  "runtime/src/package_service/tests.rs",
  "runtime/src/package_store.rs",
  "runtime/src/package_trust.rs",
  "runtime/src/package_worker_broker.rs",
  "runtime/src/package_worker_protocol.rs",
  "runtime/src/package_worker_supervisor/authority.rs",
  "runtime/src/package_worker_supervisor/calls_dispatch.rs",
  "runtime/src/package_worker_supervisor/tests/api/opaque_roots.rs",
  "runtime/src/pomodoro_host.rs",
  "runtime/src/runtime_grants.rs",
  "runtime/src/store_catalog.rs",
  "runtime/src/usage_tracker/mod.rs",
  "runtime/src/usage_tracker/windows_capture.rs",
  "runtime/tests/ark_markdown_bridge_worker.rs",
  "runtime/tests/desktop_authority_socket.rs",
  "runtime/tests/engine_control_protocol.rs",
  "runtime/tests/package_worker_process_windows.rs",
  "runtime/tests/package_worker_windows.rs",
]);
const IGNORED = new Set([
  ".git",
  ".tmp",
  "build",
  "coverage",
  "dist",
  "dist-electron",
  "node_modules",
  "release",
  "target",
  "tools",
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
for (const file of await collect(ROOT)) {
  const lines = lineCount(await readFile(file, "utf8"));
  const relative = path.relative(ROOT, file).replaceAll("\\", "/");
  if (lines > SOURCE_LIMIT && !GRANDFATHERED.has(relative)) {
    violations.push(`${relative}: ${lines} lines (max ${SOURCE_LIMIT})`);
  }
}

if (violations.length) {
  console.error(`source size check failed (${violations.length} file(s))`);
  console.error(violations.sort().join("\n"));
  process.exit(1);
}

const debt = [];
for (const file of await collect(ROOT)) {
  const relative = path.relative(ROOT, file).replaceAll("\\", "/");
  if (GRANDFATHERED.has(relative)) {
    const lines = lineCount(await readFile(file, "utf8"));
    if (lines > SOURCE_LIMIT) debt.push(`${relative}: ${lines} lines`);
  }
}
console.log(
  `source size check passed (${debt.length} grandfathered file(s) over ${SOURCE_LIMIT} lines)`,
);
if (debt.length) console.log(`baseline debt:\n${debt.sort().join("\n")}`);
