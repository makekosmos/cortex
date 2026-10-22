import assert from "node:assert/strict";
import test from "node:test";
import {
  validateDependencyGraph,
  validateArkNetSourceBoundary,
  validateDesktopFeatureTree,
  validateMobileFeatureTree,
} from "./check-ark-dependency-graph.mjs";

function metadata(overrides = {}) {
  const graph = {
    "ark-domain": [],
    "ark-net-policy": [],
    "ark-sync-contracts": ["ark-domain"],
    "ark-application": ["ark-domain"],
    "ark-store": ["ark-application", "ark-domain"],
    "ark-sync": ["ark-application", "ark-domain", "ark-sync-contracts"],
    "ark-net": ["ark-domain", "ark-net-policy", "ark-sync-contracts"],
    "ark-core": [
      "ark-application",
      "ark-domain",
      "ark-net",
      "ark-store",
      "ark-sync",
      "ark-sync-contracts",
    ],
    ...overrides,
  };
  return {
    packages: Object.entries(graph).map(([name, dependencies]) => ({
      name,
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
    .find((pkg) => pkg.name === "ark-core")
    .dependencies.push({
      name: "iroh",
      kind: "dev",
    });
  assert.deepEqual(validateDependencyGraph(graph), []);
});

test("rejects an injected forbidden dependency edge", () => {
  const graph = metadata({
    "ark-sync": ["ark-application", "ark-core", "ark-domain", "ark-sync-contracts"],
  });
  assert.deepEqual(validateDependencyGraph(graph), [
    "ark-sync: internal normal dependencies are [ark-application, ark-core, ark-domain, ark-sync-contracts], expected [ark-application, ark-domain, ark-sync-contracts]",
  ]);
});

test("rejects storage contracts and SQLite imports from ark-net source", () => {
  assert.deepEqual(
    validateArkNetSourceBoundary([
      {
        path: "iroh.rs",
        source:
          "use ark_sync::SyncRuntimeStorage; use ark_application::IntegrationReplicationBackend; use ark_sync::StorageBackedOutboundIntegrationFrameAuthorizer; use rusqlite::Connection;",
      },
    ]),
    [
      "ark-net iroh.rs: forbidden storage dependency SyncRuntimeStorage",
      "ark-net iroh.rs: forbidden storage dependency IntegrationReplicationBackend",
      "ark-net iroh.rs: forbidden storage dependency StorageBackedOutboundIntegrationFrameAuthorizer",
      "ark-net iroh.rs: forbidden storage dependency rusqlite",
    ],
  );
});

test("rejects ARK workspace crates missing from the dependency whitelist", () => {
  const graph = metadata();
  graph.packages.push({
    name: "ark-future-adapter",
    dependencies: [{ name: "ark-core", kind: null }],
  });
  assert.deepEqual(validateDependencyGraph(graph), [
    "ark-future-adapter: ARK workspace package is missing from the dependency graph",
  ]);
});

test("accepts desktop networking features and rejects them in the mobile graph", () => {
  assert.deepEqual(validateDesktopFeatureTree(desktopFeatures), []);
  assert.deepEqual(validateMobileFeatureTree('iroh feature "tls-ring"'), []);
  assert.deepEqual(validateMobileFeatureTree(desktopFeatures), [
    "mobile iroh graph enables desktop-only features: fast-apple-datapath, metrics, portmapper",
  ]);
});
