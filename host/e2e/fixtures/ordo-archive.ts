import { execFileSync } from "node:child_process";
import { createHash } from "node:crypto";
import fs from "node:fs";
import path from "node:path";

const DIGEST = "401781dd9ad396455068546926fc95a16ea772cca42d967114cf60b746afe3e3";
const SOURCE_COMMIT = "20790f6ba3f64a24e9ed5162d48f53e409b4e884";
const EXPECTED_MANIFEST = {
  schema_version: 2,
  id: "com.kosmos.focus",
  name: "Ordo",
  version: "0.1.3",
  kind: "app",
  engine_api: ">=1.0.0",
  entrypoint: "dist/index.html",
  icon: "icon.png",
  publisher: "kosmos",
  permissions: [
    {
      capability: "ark.read",
      scopes: [
        "focus.list_blocklists",
        "focus.get_active_state",
        "focus.resolve_blocklist_domains",
        "pomodoro.get_state",
      ],
    },
    {
      capability: "ark.write",
      scopes: [
        "focus.upsert_blocklist",
        "focus.delete_blocklist",
        "focus.set_active_state",
        "pomodoro.start",
        "pomodoro.pause",
        "pomodoro.resume",
        "pomodoro.stop",
      ],
    },
  ],
  targets: [{ runtime: "kosmos-host", os: ["windows"] }],
  data: { access: [], defines: [], mappings: [] },
};

const isUnsafeEntry = (entry: string) => {
  const pathWithoutDirectoryMarker = entry.replace(/\/$/, "");
  const segments = pathWithoutDirectoryMarker.split("/");
  return (
    entry.includes("\\") ||
    entry.startsWith("/") ||
    /^[A-Za-z]:/.test(entry) ||
    pathWithoutDirectoryMarker.includes("//") ||
    segments.includes(".") ||
    segments.includes("..")
  );
};

export function ordoArchive(root: string, repositoryRoot: string) {
  const repository = path.join(repositoryRoot, "focus");
  const source = path.join(repository, "release", "ordo-0.1.3.kspkg");
  if (!fs.existsSync(source)) throw new Error(`Ordo release archive not found: ${source}`);
  if (
    execFileSync("git", ["rev-parse", "HEAD"], { cwd: repository, encoding: "utf8" }).trim() !==
    SOURCE_COMMIT
  )
    throw new Error("Ordo checkout does not match the reviewed package revision");
  const file = path.join(root, path.basename(source));
  fs.copyFileSync(source, file);
  if (createHash("sha256").update(fs.readFileSync(file)).digest("hex") !== DIGEST)
    throw new Error("Ordo release archive digest does not match the reviewed artifact");

  const tar = (...args: string[]) =>
    execFileSync("tar", args, { cwd: root, encoding: "utf8", stdio: "pipe" });
  const entries = tar("-tf", file).split(/\r?\n/).filter(Boolean);
  if (entries.filter((entry) => entry === "manifest.json").length !== 1)
    throw new Error("Ordo archive must contain exactly one manifest.json");
  if (entries.some(isUnsafeEntry)) throw new Error("Ordo archive contains an unsafe path");
  if (new Set(entries.map((entry) => entry.toLowerCase())).size !== entries.length)
    throw new Error("Ordo archive contains duplicate paths");
  for (const required of ["manifest.json", "icon.png", "dist/index.html"])
    if (!entries.includes(required)) throw new Error(`Ordo archive is missing ${required}`);
  for (const entry of entries)
    if (
      entry !== "manifest.json" &&
      entry !== "icon.png" &&
      entry !== "dist/" &&
      !entry.startsWith("dist/")
    )
      throw new Error(`Ordo archive has unexpected entry: ${entry}`);

  // SAFETY: the archive has exactly one manifest.json and its bytes are JSON-encoded.
  const manifest = JSON.parse(tar("-xOf", file, "manifest.json")) as typeof EXPECTED_MANIFEST;
  if (JSON.stringify(manifest) !== JSON.stringify(EXPECTED_MANIFEST))
    throw new Error("Ordo archive manifest does not match the reviewed v2 contract");
  return { file, manifest };
}
