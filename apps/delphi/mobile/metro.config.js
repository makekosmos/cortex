const { getDefaultConfig } = require("expo/metro-config");
const path = require("path");

const projectRoot = __dirname;
const monorepoRoot = path.resolve(projectRoot, "../../..");

const config = getDefaultConfig(projectRoot);

// Tell Metro where to find node_modules (local + monorepo root for hoisted deps)
config.resolver.nodeModulesPaths = [
  path.resolve(projectRoot, "node_modules"),
  path.resolve(monorepoRoot, "node_modules"),
];

// Follow bun workspace symlinks
config.resolver.unstable_enableSymlinks = true;

// Only watch project root, NOT the entire monorepo (fixes jsc-safe-url crash)
config.projectRoot = projectRoot;

module.exports = config;
