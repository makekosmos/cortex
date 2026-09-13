import { readFile } from "node:fs/promises";
import path from "node:path";

export const NAMES = ["imago", "arca-sdk"];
const SHA = /^[0-9a-f]{40}$/;
const fail = (message) => {
  throw new Error(message);
};
function validatePin(pin, name) {
  if (!pin || typeof pin !== "object") fail(`workspace.${name} must be an object`);
  if (typeof pin.repository !== "string" || !pin.repository)
    fail(`workspace.${name}.repository is required`);
  if (!SHA.test(pin.commit)) fail(`workspace.${name}.commit must be a 40-character lowercase SHA`);
  if (!pin.package || typeof pin.package !== "object")
    fail(`workspace.${name}.package is required`);
  for (const key of ["name", "version", "integrity"])
    if (typeof pin.package[key] !== "string" || !pin.package[key])
      fail(`workspace.${name}.package.${key} is required`);
  if (pin.package.integrity !== `git:${pin.commit}`)
    fail(`workspace.${name}.package.integrity must be git:${pin.commit}`);
  return pin;
}
export async function loadWorkspace(root) {
  const packageJson = JSON.parse(await readFile(path.join(root, "package.json"), "utf8")),
    pins = packageJson.kosmos?.workspace;
  if (!pins || typeof pins !== "object") fail("package.json is missing kosmos.workspace pins");
  for (const name of NAMES) validatePin(pins[name], name);
  return { packageJson, pins };
}
export function resolveMode(env = process.env, base = process.cwd()) {
  if (env.KOSMOS_WORKSPACE_MODE && env.KOSMOS_WORKSPACE_MODE !== "local")
    fail("KOSMOS_WORKSPACE_MODE must be omitted or local");
  if (env.KOSMOS_WORKSPACE_MODE !== "local")
    return {
      name: "pinned",
      paths: Object.fromEntries(
        NAMES.map((name) => [name, path.resolve(base, ".tmp", "workspace", name)]),
      ),
    };
  const sources = { imago: env.KOSMOS_IMAGO_PATH, "arca-sdk": env.KOSMOS_ARCA_SDK_PATH };
  for (const name of NAMES)
    if (!sources[name])
      fail(
        `local mode requires ${name === "imago" ? "KOSMOS_IMAGO_PATH" : "KOSMOS_ARCA_SDK_PATH"}`,
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
