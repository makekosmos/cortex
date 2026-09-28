import { readFile } from "node:fs/promises";
import path from "node:path";

export const NAMES = ["imago", "arca-sdk"];
const SHA = /^[0-9a-f]{40}$/;
const isRecord = (value) => value !== null && Object.getPrototypeOf(value) === Object.prototype;
const isString = (value) => Object.prototype.toString.call(value) === "[object String]";
const fail = (message) => {
  throw new Error(message);
};
function validatePin(pin, name) {
  if (!isRecord(pin)) fail(`workspace.${name} must be an object`);
  if (!isString(pin.repository) || !pin.repository)
    fail(`workspace.${name}.repository is required`);
  if (!SHA.test(pin.commit)) fail(`workspace.${name}.commit must be a 40-character lowercase SHA`);
  if (!isRecord(pin.package)) fail(`workspace.${name}.package is required`);
  for (const key of ["name", "version", "integrity"])
    if (!isString(pin.package[key]) || !pin.package[key])
      fail(`workspace.${name}.package.${key} is required`);
  if (pin.package.integrity !== `git:${pin.commit}`)
    fail(`workspace.${name}.package.integrity must be git:${pin.commit}`);
  return pin;
}
export async function loadWorkspace(root) {
  const packageJson = JSON.parse(await readFile(path.join(root, "package.json"), "utf8")),
    pins = packageJson.mundus?.workspace;
  if (!isRecord(pins)) fail("package.json is missing mundus.workspace pins");
  for (const name of NAMES) validatePin(pins[name], name);
  return { packageJson, pins };
}
export function resolveMode(env = process.env, base = process.cwd()) {
  if (env.MUNDUS_WORKSPACE_MODE && env.MUNDUS_WORKSPACE_MODE !== "local")
    fail("MUNDUS_WORKSPACE_MODE must be omitted or local");
  if (env.MUNDUS_WORKSPACE_MODE !== "local")
    return {
      name: "pinned",
      paths: Object.fromEntries(
        NAMES.map((name) => [name, path.resolve(base, ".tmp", "workspace", name)]),
      ),
    };
  const sources = { imago: env.MUNDUS_IMAGO_PATH, "arca-sdk": env.MUNDUS_ARCA_SDK_PATH };
  for (const name of NAMES)
    if (!sources[name])
      fail(
        `local mode requires ${name === "imago" ? "MUNDUS_IMAGO_PATH" : "MUNDUS_ARCA_SDK_PATH"}`,
      );
  return {
    name: "local",
    paths: Object.fromEntries(
      NAMES.map((name) => [name, path.resolve(base, ".tmp", "workspace", name)]),
    ),
    sources: Object.fromEntries(NAMES.map((name) => [name, path.resolve(sources[name])])),
  };
}
export const resolveWorkspacePaths = (root, env = process.env) => resolveMode(env, root).paths;
