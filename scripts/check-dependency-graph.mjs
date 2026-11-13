#!/usr/bin/env node

import { execFileSync } from "node:child_process";
import { mkdir, readFile, readdir, writeFile } from "node:fs/promises";
import path from "node:path";
import process from "node:process";
import { fileURLToPath } from "node:url";

const ROOT = path.resolve(path.dirname(fileURLToPath(import.meta.url)), "..");
const GRAPH = {
  domain: [],
  store: ["domain"],
  sync: ["domain"],
  net: ["domain"],
  kernel: ["domain", "net", "store", "sync"],
};
const DESKTOP_ONLY_IROH_FEATURES = ["fast-apple-datapath", "metrics", "portmapper"];

const CORE_CRATES = "/core/crates/";

/**
 * Cargo metadata is parsed JSON, so a field is whatever the tool printed. The
 * graph reads two of them, and it decodes them here — at the I/O boundary —
 * instead of narrowing them again wherever they are used: past this point a
 * package name and its manifest path are text.
 */
function decodeText(value, field) {
  if (Object.prototype.toString.call(value) !== "[object String]") {
    throw new Error(`cargo metadata: ${field} is not a string`);
  }
  return value;
}

/** Decode one cargo metadata package into what the graph reads: name and path. */
function decodePackage(entry, index) {
  const field = `packages[${index}]`;
  return {
    name: decodeText(entry.name, `${field}.name`),
    // `\` -> `/`, so a manifest path reads the same on Windows and on CI.
    manifestPath: decodeText(entry.manifest_path, `${field}.manifest_path`).replaceAll("\\", "/"),
    // Edges keep cargo's shape: the graph compares their names and `kind`.
    dependencies: entry.dependencies,
  };
}

function decodePackages(metadata) {
  return new Map(
    metadata.packages.map((entry, index) => {
      const pkg = decodePackage(entry, index);
      return [pkg.name, pkg];
    }),
  );
}

/** Core crates are the packages under `core/crates/`; the graph describes them. */
function isCoreCrate(pkg) {
  return pkg.manifestPath.includes(CORE_CRATES);
}

export function validateDependencyGraph(metadata) {
  const packages = decodePackages(metadata);
  const errors = [];
  const unclassifiedPackages = [...packages.values()]
    .filter((pkg) => isCoreCrate(pkg) && !Object.hasOwn(GRAPH, pkg.name))
    .map((pkg) => pkg.name)
    .sort();
  for (const name of unclassifiedPackages) {
    errors.push(`${name}: core workspace package is missing from the dependency graph`);
  }
  for (const [name, allowed] of Object.entries(GRAPH)) {
    const pkg = packages.get(name);
    if (!pkg) {
      errors.push(`${name}: workspace package missing from cargo metadata`);
      continue;
    }
    const actual = pkg.dependencies
      .filter((dependency) => dependency.kind === null && packages.has(dependency.name))
      .map((dependency) => dependency.name)
      .sort();
    const expected = [...allowed].sort();
    if (JSON.stringify(actual) !== JSON.stringify(expected)) {
      errors.push(
        `${name}: internal normal dependencies are [${actual.join(", ")}], expected [${expected.join(", ")}]`,
      );
    }
  }
  return errors;
}

export function validateArkNetSourceBoundary(sources) {
  const forbidden = [
    "SyncRuntimeStorage",
    "IntegrationReplicationBackend",
    "StorageBackedOutboundIntegrationFrameAuthorizer",
    "rusqlite",
  ];
  return sources.flatMap(({ path: sourcePath, source }) =>
    forbidden
      .filter((dependency) => new RegExp(`\\b${dependency}\\b`).test(source))
      .map((dependency) => `net ${sourcePath}: forbidden storage dependency ${dependency}`),
  );
}

export function validateMobileFeatureTree(tree) {
  const enabled = DESKTOP_ONLY_IROH_FEATURES.filter((feature) =>
    tree.includes(`iroh feature "${feature}"`),
  );
  return enabled.length
    ? [`mobile iroh graph enables desktop-only features: ${enabled.join(", ")}`]
    : [];
}

export function validateDesktopFeatureTree(tree) {
  const missing = DESKTOP_ONLY_IROH_FEATURES.filter(
    (feature) => !tree.includes(`iroh feature "${feature}"`),
  );
  return missing.length
    ? [`desktop-net graph is missing expected iroh features: ${missing.join(", ")}`]
    : [];
}

function option(name) {
  const index = process.argv.indexOf(name);
  return index === -1 ? undefined : process.argv[index + 1];
}

function cargo(args) {
  return execFileSync("cargo", args, {
    cwd: ROOT,
    encoding: "utf8",
    stdio: ["ignore", "pipe", "inherit"],
  }).trim();
}

async function rustSources(directory, relativeTo) {
  const entries = await readdir(directory, { withFileTypes: true });
  const sources = await Promise.all(
    entries.map(async (entry) => {
      const absolutePath = path.join(directory, entry.name);
      if (entry.isDirectory()) return rustSources(absolutePath, relativeTo);
      if (!entry.isFile() || !entry.name.endsWith(".rs")) return [];
      return [
        {
          path: path.relative(relativeTo, absolutePath),
          source: await readFile(absolutePath, "utf8"),
        },
      ];
    }),
  );
  return sources.flat();
}

async function main() {
  const profile = option("--profile");
  const target = option("--target");
  if (!["desktop", "mobile"].includes(profile) || !target) {
    throw new Error(
      "usage: node scripts/check-dependency-graph.mjs --profile desktop|mobile --target <triple>",
    );
  }

  const metadata = JSON.parse(cargo(["metadata", "--locked", "--no-deps", "--format-version=1"]));
  const sourceBoundary = validateArkNetSourceBoundary(
    await rustSources(path.join(ROOT, "core/crates/net/src"), ROOT),
  );
  const errors = [...validateDependencyGraph(metadata), ...sourceBoundary];
  const treeArgs =
    profile === "desktop"
      ? [
          "tree",
          "--locked",
          "-p",
          "kernel",
          "--target",
          target,
          "--features",
          "desktop-net",
          "-e",
          "features",
          "-i",
          "iroh",
        ]
      : [
          "tree",
          "--locked",
          "-p",
          "engine-mobile",
          "--target",
          target,
          "-e",
          "features",
          "-i",
          "iroh",
        ];
  const featureTree = cargo(treeArgs);
  errors.push(
    ...(profile === "desktop"
      ? validateDesktopFeatureTree(featureTree)
      : validateMobileFeatureTree(featureTree)),
  );
  if (errors.length) throw new Error(errors.join("\n"));

  const packages = decodePackages(metadata);
  const artifact = {
    profile,
    target,
    internalNormalDependencies: Object.fromEntries(
      Object.keys(GRAPH).map((name) => [
        name,
        packages
          .get(name)
          .dependencies.filter(
            (dependency) => dependency.kind === null && packages.has(dependency.name),
          )
          .map((dependency) => dependency.name)
          .sort(),
      ]),
    ),
    irohFeatureTree: featureTree,
  };
  const output = path.join(ROOT, "target", "ark-dependency-graph", `${profile}-${target}.json`);
  await mkdir(path.dirname(output), { recursive: true });
  await writeFile(output, `${JSON.stringify(artifact, null, 2)}\n`);
  console.log(`ARK dependency graph passed: ${profile} ${target}`);
  console.log(`Graph artifact: ${path.relative(ROOT, output)}`);
}

if (process.argv[1] && path.resolve(process.argv[1]) === fileURLToPath(import.meta.url)) {
  main().catch((error) => {
    console.error(error.message);
    process.exitCode = 1;
  });
}
