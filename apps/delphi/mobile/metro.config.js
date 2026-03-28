const { getDefaultConfig } = require("expo/metro-config");
const path = require("path");

const projectRoot = __dirname;
const monorepoRoot = path.resolve(projectRoot, "../../..");

const config = getDefaultConfig(projectRoot);

// Monorepo: resolve from both local and root node_modules
config.resolver.nodeModulesPaths = [
  path.resolve(projectRoot, "node_modules"),
  path.resolve(monorepoRoot, "node_modules"),
];

// Follow symlinks (bun hoists into .bun/ via symlinks)
config.resolver.unstable_enableSymlinks = true;

// Watch monorepo root so Metro sees hoisted packages
config.watchFolders = [monorepoRoot];

module.exports = config;
