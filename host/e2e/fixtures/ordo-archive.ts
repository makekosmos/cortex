import { execFileSync } from "node:child_process";
import { createHash } from "node:crypto";
import fs from "node:fs";
import path from "node:path";
import { gitEnv } from "../../../scripts/git-env.mjs";
import { readZip } from "../../../desktop/scripts/zip-utils.mjs";

const DIGEST = "a546cd6085a9092a4ef50dd3db9a1ad8ad6f47069f8c0da03f52735d3d7a6e9a";
const SOURCE_COMMIT = "452b7be298f7f080fc8f2f80618e8e70651f1b99";
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
  targets: [{ runtime: "kosmos-host", os: ["windows", "linux"] }],
  data: { access: [], defines: [], mappings: [] },
};

const gitExecutable =
  process.platform === "win32"
    ? path.join(process.env.ProgramFiles ?? "C:\\Program Files", "Git", "cmd", "git.exe")
    : "git";

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

// Package v1 archives are plain ZIPs; the shared desktop reader is portable,
// unlike `tar` (bsdtar on Windows auto-detects ZIP, GNU tar on Linux does not).
const zipEntry = (file: string, name: string) => readZip(file).find((entry) => entry.name === name);

export function ordoArchive(root: string, repositoryRoot: string) {
  const repository = path.join(repositoryRoot, "ordo");
  const source = path.join(repository, "release", "ordo-0.1.3.kspkg");
  if (!fs.existsSync(source)) throw new Error(`Ordo release archive not found: ${source}`);
  if (
    execFileSync(gitExecutable, ["rev-parse", "HEAD"], {
      cwd: repository,
      encoding: "utf8",
      env: gitEnv(),
    }).trim() !== SOURCE_COMMIT
  )
    throw new Error("Ordo checkout does not match the reviewed package revision");
  const file = path.join(root, path.basename(source));
  fs.copyFileSync(source, file);
  if (createHash("sha256").update(fs.readFileSync(file)).digest("hex") !== DIGEST)
    throw new Error("Ordo release archive digest does not match the reviewed artifact");

  const entries = readZip(file).map((entry) => entry.name);
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
  const manifestEntry = zipEntry(file, "manifest.json");
  if (!manifestEntry) throw new Error("Ordo archive is missing manifest.json");
  // SAFETY: the pinned archive manifest is compared field-for-field with the expected contract.
  const manifest = JSON.parse(manifestEntry.data.toString("utf8")) as typeof EXPECTED_MANIFEST;
  if (JSON.stringify(manifest) !== JSON.stringify(EXPECTED_MANIFEST))
    throw new Error("Ordo archive manifest does not match the reviewed v2 contract");
  return { file, manifest };
}
