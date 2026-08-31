import { execFileSync } from "node:child_process";
import { createHash } from "node:crypto";
import fs from "node:fs";
import path from "node:path";

const SOURCE_COMMIT = "57aae83d4bc80c5d0342a6b92072c59840d54dd4";
const DIGEST = "2a1c001a2240275a4f8fa5307cce1faf1132442f8a51e98bbbbd7de1800ffac6";
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
  const repository = path.join(repositoryRoot, "dictation");
  const file = path.join(root, "dictation-0.2.2.kspkg");
  fs.writeFileSync(
    file,
    execFileSync("git", ["show", `${SOURCE_COMMIT}:tests/fixtures/dictation-0.2.2.kspkg`], {
      cwd: repository,
      maxBuffer: 10 * 1024 * 1024,
      stdio: ["ignore", "pipe", "pipe"],
    }),
  );
  if (createHash("sha256").update(fs.readFileSync(file)).digest("hex") !== DIGEST)
    throw new Error("Dictation release archive digest does not match the reviewed artifact");
  const tar = (...args: string[]) =>
    execFileSync("tar", args, { cwd: root, encoding: "utf8", stdio: "pipe" });
  const entries = tar("-tf", file).split(/\r?\n/).filter(Boolean);
  if (entries.filter((entry) => entry === "manifest.json").length !== 1)
    throw new Error("Dictation archive must contain exactly one manifest.json");
  for (const entry of entries) {
    const pathWithoutDirectoryMarker = entry.replace(/\/$/, "");
    const segments = pathWithoutDirectoryMarker.split("/");
    if (
      entry.includes("\\") ||
      entry.startsWith("/") ||
      /^[A-Za-z]:/.test(entry) ||
      pathWithoutDirectoryMarker.includes("//") ||
      segments.includes(".") ||
      segments.includes("..")
    ) {
      throw new Error(`Dictation archive contains an unsafe path: ${entry}`);
    }
  }
  if (new Set(entries.map((entry) => entry.toLowerCase())).size !== entries.length)
    throw new Error("Dictation archive contains duplicate paths");
  for (const required of ["manifest.json", "compatibility.json", "icon.png", "dist/index.html"])
    if (!entries.includes(required)) throw new Error(`Dictation archive is missing ${required}`);
  for (const entry of entries) {
    if (
      entry !== "manifest.json" &&
      entry !== "compatibility.json" &&
      entry !== "icon.png" &&
      entry !== "dist/" &&
      !entry.startsWith("dist/")
    ) {
      throw new Error(`Dictation archive has unexpected entry: ${entry}`);
    }
  }
  // SAFETY: the archive has exactly one manifest.json and its bytes are JSON-encoded.
  const manifest = JSON.parse(tar("-xOf", file, "manifest.json")) as typeof EXPECTED_MANIFEST;
  if (JSON.stringify(manifest) !== JSON.stringify(EXPECTED_MANIFEST))
    throw new Error("Dictation archive manifest does not match the reviewed v2 contract");
  return { file, manifest };
}
