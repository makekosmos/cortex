import { execFileSync } from "node:child_process";
import { createHash } from "node:crypto";
import fs from "node:fs";
import path from "node:path";

const DIGEST = "8ed0d8b95c3c84f9a50b0efecb1005bdfb07d448dd23f9e83cdcc8625066aae5";
const EXPECTED_MANIFEST = {
  schema_version: 2,
  id: "com.kosmos.dictation",
  name: "Dictation",
  version: "0.2.2",
  kind: "app",
  engine_api: ">=1.0.0",
  entrypoint: "dist/index.html",
  icon: "icon.png",
  publisher: "kosmos",
  permissions: [
    {
      capability: "ark.read",
      scopes: ["dictation.get_state", "dictation.get_config", "dictation.list_local_models"],
    },
    {
      capability: "ark.write",
      scopes: ["dictation.update_config", "dictation.start_recording", "dictation.cancel"],
    },
  ],
  targets: [{ runtime: "kosmos-host", os: ["windows"] }],
  data: { access: [], defines: [], mappings: [] },
};

export function dictationArchive(root: string, repositoryRoot: string) {
  const source = path.join(repositoryRoot, "dictation", "release", "dictation-0.2.2.kspkg");
  if (!fs.existsSync(source)) throw new Error(`Dictation release archive not found: ${source}`);
  const file = path.join(root, path.basename(source));
  fs.copyFileSync(source, file);
  if (createHash("sha256").update(fs.readFileSync(file)).digest("hex") !== DIGEST)
    throw new Error("Dictation release archive digest does not match the reviewed artifact");
  const tar = (...args: string[]) =>
    execFileSync("tar", args, { cwd: root, encoding: "utf8", stdio: "pipe" });
  const entries = tar("-tf", file)
    .split(/\r?\n/)
    .filter(Boolean)
    .map((entry) => entry.replaceAll("\\", "/"));
  if (entries.filter((entry) => entry === "manifest.json").length !== 1)
    throw new Error("Dictation archive must contain exactly one manifest.json");
  if (entries.some((entry) => entry.startsWith("/") || entry.split("/").includes("..")))
    throw new Error("Dictation archive contains an unsafe path");
  for (const required of ["manifest.json", "icon.png", "dist/index.html"])
    if (!entries.includes(required)) throw new Error(`Dictation archive is missing ${required}`);
  // SAFETY: the archive has exactly one manifest.json and its bytes are JSON-encoded.
  const manifest = JSON.parse(tar("-xOf", file, "manifest.json")) as typeof EXPECTED_MANIFEST;
  if (JSON.stringify(manifest) !== JSON.stringify(EXPECTED_MANIFEST))
    throw new Error("Dictation archive manifest does not match the reviewed v2 contract");
  return { file, manifest };
}
