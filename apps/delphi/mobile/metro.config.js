const { getDefaultConfig } = require("expo/metro-config");
const path = require("path");

const projectRoot = __dirname;
const monorepoRoot = path.resolve(projectRoot, "../../..");

const config = getDefaultConfig(projectRoot);

// Lock entry point to this project only
config.projectRoot = projectRoot;
config.watchFolders = [
  projectRoot,
  path.resolve(monorepoRoot, "node_modules"),
];

// Resolve hoisted bun dependencies
config.resolver.nodeModulesPaths = [
  path.resolve(projectRoot, "node_modules"),
  path.resolve(monorepoRoot, "node_modules"),
];

// Follow bun workspace symlinks
config.resolver.unstable_enableSymlinks = true;

// Prevent HMR from crashing when it tries to resolve the monorepo root as a module
config.resolver.resolveRequest = (context, moduleName, platform) => {
  if (
    context.originModulePath === monorepoRoot ||
    context.originModulePath === monorepoRoot + "/."
  ) {
    return { type: "empty" };
  }
  return context.resolveRequest(context, moduleName, platform);
};

module.exports = config;
