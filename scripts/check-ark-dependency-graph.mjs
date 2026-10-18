#!/usr/bin/env node

import { execFileSync } from "node:child_process";
import { mkdir, readFile, readdir, writeFile } from "node:fs/promises";
import path from "node:path";
import process from "node:process";
import { fileURLToPath } from "node:url";

const ROOT = path.resolve(path.dirname(fileURLToPath(import.meta.url)), "..");
const GRAPH = {
  "ark-domain": [],
  "ark-net-policy": [],
  "ark-application": ["ark-domain"],
  "ark-store": ["ark-application", "ark-domain"],
  "ark-sync": ["ark-application", "ark-domain"],
  "ark-net": ["ark-net-policy", "ark-sync"],
  "ark-core": ["ark-application", "ark-domain", "ark-net", "ark-store", "ark-sync"],
};
const DESKTOP_ONLY_IROH_FEATURES = ["fast-apple-datapath", "metrics", "portmapper"];

export function validateDependencyGraph(metadata) {
  const packages = new Map(metadata.packages.map((pkg) => [pkg.name, pkg]));
  const errors = [];
  const unclassifiedPackages = [...packages.keys()]
    .filter((name) => name.startsWith("ark-") && !Object.hasOwn(GRAPH, name))
    .sort();
  for (const name of unclassifiedPackages) {
    errors.push(`${name}: ARK workspace package is missing from the dependency graph`);
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
    "ark_store",
    "rusqlite",
  ];
  return sources.flatMap(({ path: sourcePath, source }) =>
    forbidden
      .filter((dependency) => new RegExp(`\\b${dependency}\\b`).test(source))
      .map((dependency) => `ark-net ${sourcePath}: forbidden storage dependency ${dependency}`),
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
      "usage: node scripts/check-ark-dependency-graph.mjs --profile desktop|mobile --target <triple>",
    );
  }

  const metadata = JSON.parse(cargo(["metadata", "--locked", "--no-deps", "--format-version=1"]));
  const sourceBoundary = validateArkNetSourceBoundary(
    await rustSources(path.join(ROOT, "core/crates/ark-net/src"), ROOT),
  );
  const errors = [...validateDependencyGraph(metadata), ...sourceBoundary];
  const treeArgs =
    profile === "desktop"
      ? [
          "tree",
          "--locked",
          "-p",
          "ark-core",
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

  const packages = new Map(metadata.packages.map((pkg) => [pkg.name, pkg]));
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
