import assert from "node:assert/strict";
import test from "node:test";
import {
  validateDependencyGraph,
  validateArkNetSourceBoundary,
  validateDesktopFeatureTree,
  validateMobileFeatureTree,
} from "./check-dependency-graph.mjs";

function metadata(overrides = {}) {
  const graph = {
    "domain": [],
    "store": ["domain"],
    "sync": ["domain"],
    "net": ["domain"],
    "kernel": ["domain", "net", "store", "sync"],
    ...overrides,
  };
  return {
    packages: Object.entries(graph).map(([name, dependencies]) => ({
      name,
      manifest_path: `/repo/core/crates/${name}/Cargo.toml`,
      dependencies: dependencies.map((dependency) => ({ name: dependency, kind: null })),
    })),
  };
}

const desktopFeatures = [
  'iroh feature "fast-apple-datapath"',
  'iroh feature "metrics"',
  'iroh feature "portmapper"',
].join("\n");

test("accepts the declared ARK dependency whitelist and ignores dev-only edges", () => {
  const graph = metadata();
  graph.packages
    .find((pkg) => pkg.name === "kernel")
    .dependencies.push({
      name: "iroh",
      kind: "dev",
    });
  assert.deepEqual(validateDependencyGraph(graph), []);
});

test("rejects an injected forbidden dependency edge", () => {
  const graph = metadata({
    "sync": ["kernel", "domain"],
  });
  assert.deepEqual(validateDependencyGraph(graph), [
    "sync: internal normal dependencies are [domain, kernel], expected [domain]",
  ]);
});

test("leaves a parameter named store alone", () => {
  assert.deepEqual(
    validateArkNetSourceBoundary([
      {
        path: "iroh.rs",
        source: "fn send(store: &IrohStore) { let _ = store; }",
      },
    ]),
    [],
  );
});

test("rejects storage contracts and SQLite imports from net source", () => {
  assert.deepEqual(
    validateArkNetSourceBoundary([
      {
        path: "iroh.rs",
        source:
          "use sync::SyncRuntimeStorage; use ark_application::IntegrationReplicationBackend; use sync::StorageBackedOutboundIntegrationFrameAuthorizer; use rusqlite::Connection;",
      },
    ]),
    [
      "net iroh.rs: forbidden storage dependency SyncRuntimeStorage",
      "net iroh.rs: forbidden storage dependency IntegrationReplicationBackend",
      "net iroh.rs: forbidden storage dependency StorageBackedOutboundIntegrationFrameAuthorizer",
      "net iroh.rs: forbidden storage dependency rusqlite",
    ],
  );
});

test("rejects core crates missing from the dependency whitelist", () => {
  const graph = metadata();
  graph.packages.push({
    name: "kernel-future",
    manifest_path: "/repo/core/crates/kernel-future/Cargo.toml",
    dependencies: [{ name: "kernel", kind: null }],
  });
  assert.deepEqual(validateDependencyGraph(graph), [
    "kernel-future: core workspace package is missing from the dependency graph",
  ]);
});

test("ignores workspace packages outside core/crates", () => {
  const graph = metadata();
  for (const name of ["engine", "engine-mobile", "pe-version-info"]) {
    graph.packages.push({
      name,
      manifest_path: `/repo/runtime/crates/${name}/Cargo.toml`,
      dependencies: [{ name: "kernel", kind: null }],
    });
  }
  assert.deepEqual(validateDependencyGraph(graph), []);
});

test("accepts desktop networking features and rejects them in the mobile graph", () => {
  assert.deepEqual(validateDesktopFeatureTree(desktopFeatures), []);
  assert.deepEqual(validateMobileFeatureTree('iroh feature "tls-ring"'), []);
  assert.deepEqual(validateMobileFeatureTree(desktopFeatures), [
    "mobile iroh graph enables desktop-only features: fast-apple-datapath, metrics, portmapper",
  ]);
});
