import { execFileSync } from "node:child_process";
import { createHash } from "node:crypto";
import fs from "node:fs";
import path from "node:path";
import { isDeepStrictEqual } from "node:util";
import { gitEnv } from "../../../scripts/git-env.mjs";
import { readZip } from "../../../desktop/scripts/zip-utils.mjs";

// Windows replays the reviewed checked-in fixture; Dictation 0.2.2 predates
// the package-worker contract and stays the win32 pin.
const WINDOWS_SOURCE_COMMIT = "57aae83d4bc80c5d0342a6b92072c59840d54dd4";
const WINDOWS_ARCHIVE_PATH = "tests/fixtures/dictation-0.2.2.kspkg";
const WINDOWS_ARCHIVE_SHA256 = "2a1c001a2240275a4f8fa5307cce1faf1132442f8a51e98bbbbd7de1800ffac6";
const WINDOWS_EXPECTED_MANIFEST = {
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

// Linux installs Dictation through the kosmos-host target only (package
// workers are unsupported off Windows). The artifact is built from the
// pinned source commit with `bun run package:kspkg` in the Dictation
// checkout; release/ is gitignored there, so the digest below is the
// reviewed pin. The archive still carries the declared Windows worker
// entrypoint (worker/dictation-worker.exe, an ELF there) because the
// packager always copies the built worker to that path — it is never
// launched off Windows.
const LINUX_SOURCE_COMMIT = "b37e8cdb1ffd60762138606cd9dd3491815ee4f1";
const LINUX_ARCHIVE_PATH = "release/dictation-0.2.5.kspkg";
const LINUX_ARCHIVE_SHA256 = "7d1c0494b408cbf780802b26f8e56fd874ee5b06be466d523e13892fb31831cb";

const isWindows = process.platform === "win32";
const SOURCE_COMMIT = isWindows ? WINDOWS_SOURCE_COMMIT : LINUX_SOURCE_COMMIT;
export const DICTATION_ARCHIVE_SHA256 = isWindows ? WINDOWS_ARCHIVE_SHA256 : LINUX_ARCHIVE_SHA256;

const REQUIRED_ENTRIES = ["manifest.json", "compatibility.json", "icon.png", "dist/index.html"];

type Manifest = { id: string; version: string; [key: string]: JsonValue };
type JsonValue =
  | string
  | number
  | boolean
  | null
  | readonly JsonValue[]
  | { readonly [key: string]: JsonValue };

const gitExecutable = isWindows
  ? path.join(process.env.ProgramFiles ?? "C:\\Program Files", "Git", "cmd", "git.exe")
  : "git";
const digest = (value: string | Buffer) => createHash("sha256").update(value).digest("hex");

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

export function dictationArchive(root: string, repositoryRoot: string) {
  // Reviewed sources: Windows 57aae83d4bc80c5d0342a6b92072c59840d54dd4,
  // Linux b37e8cdb1ffd60762138606cd9dd3491815ee4f1.
  const sourceRoot = path.join(repositoryRoot, "dictation");
  const file = path.join(
    root,
    path.basename(isWindows ? WINDOWS_ARCHIVE_PATH : LINUX_ARCHIVE_PATH),
  );
  if (isWindows) {
    fs.writeFileSync(
      file,
      execFileSync(gitExecutable, ["show", `${SOURCE_COMMIT}:${WINDOWS_ARCHIVE_PATH}`], {
        cwd: sourceRoot,
        maxBuffer: 10 * 1024 * 1024,
        stdio: ["ignore", "pipe", "pipe"],
        env: gitEnv(),
      }),
    );
  } else {
    const checkout = execFileSync(gitExecutable, ["rev-parse", "HEAD"], {
      cwd: sourceRoot,
      encoding: "utf8",
      stdio: "pipe",
      env: gitEnv(),
    }).trim();
    if (checkout !== LINUX_SOURCE_COMMIT)
      throw new Error(
        `Dictation checkout ${checkout} does not match reviewed source ${LINUX_SOURCE_COMMIT}`,
      );
    const source = path.join(sourceRoot, LINUX_ARCHIVE_PATH);
    if (!fs.existsSync(source))
      throw new Error(
        `Dictation release archive not found: ${source} (run \`bun run package:kspkg\` in the Dictation checkout)`,
      );
    fs.copyFileSync(source, file);
  }
  if (digest(fs.readFileSync(file)) !== DICTATION_ARCHIVE_SHA256)
    throw new Error("Dictation release archive digest does not match the reviewed artifact");

  const zip = readZip(file);
  const entries = zip.map((entry) => entry.name);
  if (entries.filter((entry) => entry === "manifest.json").length !== 1)
    throw new Error("Dictation archive must contain exactly one manifest.json");
  if (entries.some(isUnsafeEntry)) throw new Error("Dictation archive contains an unsafe path");
  if (new Set(entries.map((entry) => entry.toLowerCase())).size !== entries.length)
    throw new Error("Dictation archive contains duplicate paths");
  for (const required of REQUIRED_ENTRIES)
    if (!entries.includes(required)) throw new Error(`Dictation archive is missing ${required}`);

  const zipEntry = (name: string) => {
    const entry = zip.find((candidate) => candidate.name === name);
    if (!entry) throw new Error(`Dictation archive is missing ${name}`);
    return entry.data;
  };
  // SAFETY: the signed archive contains exactly one validated manifest object.
  const manifest = JSON.parse(zipEntry("manifest.json").toString("utf8")) as Manifest;
  // Declared worker entrypoints come from the manifest targets, not a
  // hard-coded name; the same check holds on both pins.
  // SAFETY: targets comes from the signed archive manifest validated above.
  const workerEntrypoints = (
    manifest.targets as readonly { runtime?: string; entrypoint?: string }[]
  )
    .flatMap((target) => (target.runtime === "worker" ? [target.entrypoint] : []))
    .filter((entrypoint): entrypoint is string => entrypoint !== undefined);
  for (const entrypoint of workerEntrypoints)
    if (!entries.includes(entrypoint))
      throw new Error(`Dictation archive is missing declared worker entrypoint ${entrypoint}`);
  for (const entry of entries) {
    if (
      entry !== "manifest.json" &&
      entry !== "compatibility.json" &&
      entry !== "icon.png" &&
      entry !== "dist/" &&
      !entry.startsWith("dist/") &&
      !workerEntrypoints.includes(entry) &&
      !(entry.endsWith("/") && workerEntrypoints.some((ep) => ep.startsWith(entry)))
    ) {
      throw new Error(`Dictation archive has unexpected entry: ${entry}`);
    }
  }
  if (isWindows) {
    if (JSON.stringify(manifest) !== JSON.stringify(WINDOWS_EXPECTED_MANIFEST))
      throw new Error("Dictation archive manifest does not match the reviewed v2 contract");
  } else {
    // SAFETY: package.manifest.json at the pinned Git commit is a reviewed manifest object.
    const reviewed = JSON.parse(
      execFileSync(gitExecutable, ["show", `${SOURCE_COMMIT}:package.manifest.json`], {
        cwd: sourceRoot,
        encoding: "utf8",
        stdio: "pipe",
        env: gitEnv(),
      }),
    ) as Manifest;
    if (!isDeepStrictEqual(manifest, reviewed))
      throw new Error(`Dictation archive manifest does not match source commit ${SOURCE_COMMIT}`);
  }
  return { file, manifest };
}
