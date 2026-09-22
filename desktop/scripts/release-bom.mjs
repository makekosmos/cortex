import { readFile } from "node:fs/promises";
import path from "node:path";
import {
  documentHash,
  fail,
  integer,
  readJson,
  safeId,
  semver,
  sha256,
} from "./package-release-utils.mjs";

export const RELEASE_BOM_SCHEMA_VERSION = 1;
const COMMIT = /^[0-9a-f]{40}$/;

function engineCompatible(requirement, current) {
  if (requirement === current) return true;
  const minimum = /^>=(\d+)\.(\d+)\.(\d+)$/.exec(requirement);
  if (!minimum) return false;
  const actual = current.split(".").map(Number);
  return (
    actual[0] === Number(minimum[1]) &&
    (actual[1] > Number(minimum[2]) ||
      (actual[1] === Number(minimum[2]) && actual[2] >= Number(minimum[3])))
  );
}

function object(value, name) {
  if (!value || Object.prototype.toString.call(value) !== "[object Object]")
    fail(`${name} must be an object`);
  return value;
}

function string(value, name) {
  if (Object.prototype.toString.call(value) !== "[object String]" || !value)
    fail(`${name} must be a non-empty string`);
  return value;
}

function commit(value, name) {
  if (Object.prototype.toString.call(value) !== "[object String]" || !COMMIT.test(value))
    fail(`${name} must be a 40-character lowercase commit`);
  return value;
}

function validateArtifacts(value) {
  if (value === undefined) return;
  if (!Array.isArray(value)) fail("artifacts must be an array");
  const names = new Set();
  for (const [index, artifact] of value.entries()) {
    object(artifact, `artifacts[${index}]`);
    const name = string(artifact.name, `artifacts[${index}].name`);
    if (name === "." || name.includes("/") || name.includes("\\") || names.has(name))
      fail(`artifacts[${index}].name must be a unique file name`);
    names.add(name);
    sha256(artifact.sha256, `artifacts[${index}].sha256`);
    integer(artifact.size, `artifacts[${index}].size`);
  }
}

function rejectSecrets(value) {
  if (Array.isArray(value)) {
    for (const item of value) rejectSecrets(item);
    return;
  }
  if (!value || Object.prototype.toString.call(value) !== "[object Object]") return;
  for (const [key, item] of Object.entries(value)) {
    if (/private.?key|secret|token/i.test(key)) fail(`BOM must not contain ${key}`);
    rejectSecrets(item);
  }
}

export function validateReleaseBom(value, context) {
  object(value, "BOM");
  object(context, "validation context");
  rejectSecrets(value);
  if (value.schema_version !== RELEASE_BOM_SCHEMA_VERSION)
    fail(`schema_version must be ${RELEASE_BOM_SCHEMA_VERSION}`);
  if (value.state !== "resolved") fail("desktop builds require a resolved BOM");
  safeId(value.id, "id");

  const release = object(value.release, "release");
  safeId(release.channel, "release.channel");
  if (release.platform !== context.platform) fail("release.platform does not match build platform");
  semver(release.version, "release.version");
  if (release.version !== context.releaseVersion)
    fail("release.version does not match release-versions.json");

  const source = object(value.source, "source");
  const cortex = object(source.cortex, "source.cortex");
  string(cortex.repository, "source.cortex.repository");
  commit(cortex.commit, "source.cortex.commit");
  if (cortex.commit !== context.currentCommit) fail("source.cortex.commit does not match HEAD");

  const core = object(source.core, "source.core");
  string(core.repository, "source.core.repository");
  commit(core.commit, "source.core.commit");
  if (core.commit !== cortex.commit) fail("source.core.commit does not match source.cortex.commit");
  if (core.path !== "core/") fail("source.core.path must be the in-tree core/ subtree");
  const arkArtifact = object(core.ark_artifact, "source.core.ark_artifact");
  const arkName = string(arkArtifact.name, "source.core.ark_artifact.name");
  if (arkName !== (context.platform === "win" ? "ark-core-rpc.exe" : "ark-core-rpc"))
    fail("source.core.ark_artifact.name does not match build platform");
  sha256(arkArtifact.sha256, "source.core.ark_artifact.sha256");
  integer(arkArtifact.size, "source.core.ark_artifact.size");

  for (const name of ["arca_sdk", "imago"]) {
    const dependency = object(source[name], `source.${name}`);
    string(dependency.repository, `source.${name}.repository`);
    commit(dependency.commit, `source.${name}.commit`);
    const packagePin = object(dependency.package, `source.${name}.package`);
    string(packagePin.name, `source.${name}.package.name`);
    semver(packagePin.version, `source.${name}.package.version`);
    if (packagePin.integrity !== `git:${dependency.commit}`)
      fail(`source.${name}.package.integrity does not match its commit`);
    const workspaceName = name === "arca_sdk" ? "arca-sdk" : name;
    const workspacePin = object(context.workspace?.[workspaceName], `workspace.${workspaceName}`);
    string(workspacePin.repository, `workspace.${workspaceName}.repository`);
    commit(workspacePin.commit, `workspace.${workspaceName}.commit`);
    const workspacePackage = object(workspacePin.package, `workspace.${workspaceName}.package`);
    string(workspacePackage.name, `workspace.${workspaceName}.package.name`);
    semver(workspacePackage.version, `workspace.${workspaceName}.package.version`);
    string(workspacePackage.integrity, `workspace.${workspaceName}.package.integrity`);
    if (dependency.commit !== workspacePin.commit)
      fail(`source.${name}.commit does not match the package.json workspace pin`);
    if (dependency.repository !== workspacePin.repository)
      fail(`source.${name}.repository does not match the package.json workspace pin`);
    if (packagePin.name !== workspacePin.package.name)
      fail(`source.${name}.package.name does not match the package.json workspace pin`);
    if (packagePin.version !== workspacePin.package.version)
      fail(`source.${name}.package.version does not match the package.json workspace pin`);
    if (packagePin.integrity !== workspacePin.package.integrity)
      fail(`source.${name}.package.integrity does not match the package.json workspace pin`);
  }
  const store = object(source.store, "source.store");
  string(store.repository, "source.store.repository");
  commit(store.commit, "source.store.commit");

  const toolchain = object(source.toolchain, "source.toolchain");
  for (const name of ["pnpm", "node", "rust"]) semver(toolchain[name], `source.toolchain.${name}`);
  for (const name of ["pnpm", "node", "rust"])
    if (toolchain[name] !== context.toolchain[name])
      fail(`source.toolchain.${name} does not match the repository pin`);
  string(toolchain.target, "source.toolchain.target");

  const compatibility = object(value.compatibility, "compatibility");
  semver(compatibility.shell_api, "compatibility.shell_api");
  semver(compatibility.engine_api, "compatibility.engine_api");
  if (compatibility.shell_api !== context.api.shell)
    fail("compatibility.shell_api does not match the shell API pin");
  if (compatibility.engine_api !== context.api.engine)
    fail("compatibility.engine_api does not match the engine API pin");
  if (compatibility.package_schema !== context.api.package_manifest)
    fail("compatibility.package_schema does not match the package catalog contract");
  integer(compatibility.package_schema, "compatibility.package_schema");

  const catalog = object(value.catalog, "catalog");
  integer(catalog.sequence, "catalog.sequence");
  if (catalog.previous_sequence !== catalog.sequence - 1)
    fail("catalog.previous_sequence must immediately precede sequence");
  integer(catalog.store_sequence, "catalog.store_sequence");
  safeId(catalog.signing_key_id, "catalog.signing_key_id");

  if (!Array.isArray(value.packages) || value.packages.length === 0)
    fail("packages must be a non-empty array");
  const packageIds = new Set();
  for (const [index, packagePin] of value.packages.entries()) {
    object(packagePin, `packages[${index}]`);
    const id = safeId(packagePin.id, `packages[${index}].id`);
    if (packageIds.has(id)) fail(`duplicate package id ${id}`);
    packageIds.add(id);
    if (packagePin.manifest_id !== id) fail(`${id}.manifest_id must match id`);
    if (!["app", "source"].includes(packagePin.kind)) fail(`${id}.kind is invalid`);
    string(packagePin.repository, `${id}.repository`);
    commit(packagePin.ref, `${id}.ref`);
    if (packagePin.kind === "source" && packagePin.ref !== cortex.commit)
      fail(`${id}.ref must match source.cortex.commit`);
    semver(packagePin.version, `${id}.version`);
    string(packagePin.entrypoint, `${id}.entrypoint`);
    string(packagePin.icon, `${id}.icon`);
    if (!engineCompatible(packagePin.engine_api, compatibility.engine_api))
      fail(`${id}.engine_api is incompatible with the release`);
    const artifact = object(packagePin.artifact, `${id}.artifact`);
    const artifactName = string(artifact.name, `${id}.artifact.name`);
    if (path.basename(artifactName) !== artifactName)
      fail(`${id}.artifact.name must be a file name`);
    const artifactUrl = string(artifact.url, `${id}.artifact.url`);
    const expectedUrl = `https://github.com/makekosmos/package-index/releases/download/catalog-${catalog.sequence}/${artifactName}`;
    if (artifactUrl !== expectedUrl)
      fail(`${id}.artifact.url must match the immutable catalog release`);
    sha256(artifact.sha256, `${id}.artifact.sha256`);
    integer(artifact.size, `${id}.artifact.size`);
  }
  validateArtifacts(value.artifacts);
  return value;
}

export async function repositoryContext(root, platform, currentCommit) {
  const packageJson = await readJson(path.join(root, "package.json"));
  const workspace = packageJson.kosmos?.workspace;
  if (!workspace?.imago || !workspace?.["arca-sdk"])
    fail("package.json workspace pins are incomplete");
  const versions = await readJson(path.join(root, "desktop", "release-versions.json"));
  const toolchain = await readJson(path.join(root, "toolchain.json"));
  const cargo = await readFile(path.join(root, "runtime", "Cargo.toml"), "utf8");
  const shellApi = await readFile(path.join(root, "desktop", "electron", "kepler-api.ts"), "utf8");
  const engineApi = await readFile(
    path.join(root, "manager", "electron", "engine-client.ts"),
    "utf8",
  );
  const catalog = await readFile(
    path.join(root, "desktop", "scripts", "package-catalog.mjs"),
    "utf8",
  );
  const arkCore = await readFile(
    path.join(root, "desktop", "scripts", "ark-core-source.mjs"),
    "utf8",
  );

  const pnpm = String(packageJson.packageManager ?? "").match(/^pnpm@(\d+\.\d+\.\d+)$/)?.[1];
  const node = toolchain.node;
  const rust = toolchain.rust;
  const corePathPin = /path = "\.\.\/core\/crates\/ark-core", package = "ark-core"/.test(cargo);
  const arkCoreSource = /ARK_CORE_SOURCE\s*=\s*"core\/crates\/ark-core"/.test(arkCore);
  const shell = shellApi.match(/KEPLER_API_VERSION\s*=\s*"([^"]+)"/)?.[1];
  const engine = engineApi.match(/ENGINE_API_VERSION\s*=\s*"([^"]+)"/)?.[1];

  if (!pnpm || !node || !rust || !corePathPin || !arkCoreSource || !shell || !engine)
    fail("repository pins are incomplete or unreadable");
  if (!/manifest\.schema_version !== 2/.test(catalog))
    fail("package catalog is not pinned to manifest schema 2");

  return {
    currentCommit,
    platform,
    releaseVersion: versions[platform],
    workspace,
    toolchain: { pnpm, node, rust },
    api: { shell, engine, package_manifest: 2 },
  };
}

export async function loadReleaseBom(file, options = {}) {
  const absolute = path.resolve(file);
  const raw = await readFile(absolute);
  let value;
  try {
    value = JSON.parse(raw);
  } catch (error) {
    fail(`invalid BOM JSON: ${error.message}`);
  }
  const context =
    options.context ??
    (await repositoryContext(options.root, options.platform, options.currentCommit));
  validateReleaseBom(value, context);
  return { value, path: absolute, bytes: raw, digest: documentHash(raw) };
}
