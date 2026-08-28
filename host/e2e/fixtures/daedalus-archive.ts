import { execFileSync } from "node:child_process";
import { createHash } from "node:crypto";
import fs from "node:fs";
import path from "node:path";

type JsonValue =
  | string
  | number
  | boolean
  | null
  | readonly JsonValue[]
  | { readonly [key: string]: JsonValue };

const isObject = (value: JsonValue): value is { readonly [key: string]: JsonValue } =>
  typeof value === "object" && value !== null && !Array.isArray(value);

export function daedalusArchive(root: string, repositoryRoot: string) {
  const source = path.join(repositoryRoot, "daedalus", "release", "daedalus-0.1.0.kspkg");
  if (!fs.existsSync(source)) throw new Error(`Daedalus release archive not found: ${source}`);
  const file = path.join(root, path.basename(source));
  fs.copyFileSync(source, file);
  if (
    createHash("sha256").update(fs.readFileSync(file)).digest("hex") !==
    "dfc5e114d979f3b51b1cd3983596047a10fd9d6711aae59238c342a939087fef"
  )
    throw new Error("Daedalus release archive digest does not match the reviewed artifact");
  const tar = (...args: string[]) =>
    execFileSync("tar", args, { cwd: root, encoding: "utf8", stdio: "pipe" });
  const entries = tar("-tf", file)
    .split(/\r?\n/)
    .filter(Boolean)
    .map((entry) => entry.replaceAll("\\", "/"));
  if (entries.filter((entry) => entry === "manifest.json").length !== 1)
    throw new Error("Daedalus archive must contain exactly one manifest.json");
  if (entries.some((entry) => entry.startsWith("/") || entry.split("/").includes("..")))
    throw new Error("Daedalus archive contains an unsafe path");
  for (const required of ["manifest.json", "dist/index.html"])
    if (!entries.includes(required)) throw new Error(`Daedalus archive is missing ${required}`);
  let manifest: JsonValue;
  try {
    // SAFETY: isObject and every required manifest field are checked below.
    manifest = JSON.parse(tar("-xOf", file, "manifest.json")) as JsonValue;
  } catch (error) {
    throw new Error(`Daedalus archive manifest is not valid JSON: ${String(error)}`);
  }
  if (
    !isObject(manifest) ||
    createHash("sha256").update(JSON.stringify(manifest)).digest("hex") !==
      "9dd6ec2be3bfae4707a720afd7e4d64303e3c8270b32fafb0fd1ce6c5d213c74" ||
    manifest.schema_version !== 2 ||
    manifest.id !== "com.kosmos.daedalus" ||
    manifest.version !== "0.1.0" ||
    manifest.kind !== "app" ||
    manifest.publisher !== "kosmos" ||
    manifest.entrypoint !== "dist/index.html"
  )
    throw new Error("Daedalus archive manifest has unexpected identity or entrypoint");
  return { file, manifest: { ...manifest, id: manifest.id, version: manifest.version } };
}
