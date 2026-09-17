import { execFileSync } from "node:child_process";
import { createHash } from "node:crypto";
import fs from "node:fs";
import path from "node:path";
import { isDeepStrictEqual } from "node:util";
import { readZip } from "../../../desktop/scripts/zip-utils.mjs";

// Windows replays the reviewed checked-in fixture built from the pinned sources;
// its worker target is Windows-only, so that archive carries a PE worker.
const WINDOWS_SOURCE_COMMIT = "2689da940071ed5068226a19b28a09490b059dae";
const WINDOWS_ARCHIVE_COMMIT = "1fcd46274bfd1c9dbfcf1dc40978b1c9e8d09520";
const WINDOWS_ARCHIVE_PATH = "tests/arcadia-0.1.11.kspkg";
const WINDOWS_ARCHIVE_SHA256 = "a05ae74be8c7bafa86f4ef91d11445ec7e256b9bd27ececd35996b4f5f83be6c";
const IMAGO_COMMIT = "b1852cab9f8f08ae0b236b7759138d3117f720b8";
const ARCA_SDK_COMMIT = "21c2f5e157944e3444d3c0604da5a0e041116077";

// Linux installs Arcadia through the kosmos-host target only (package workers
// are unsupported off Windows). The artifact is built from the pinned source
// commit with `bun run package:kspkg` in the Arcadia checkout; release/ is
// gitignored there, so the digest below is the reviewed pin.
const LINUX_SOURCE_COMMIT = "ae10decb1aa91e0704302a26bb19df031396bd79";
const LINUX_ARCHIVE_PATH = "release/arcadia-0.1.11.kspkg";
const LINUX_ARCHIVE_SHA256 = "a43c40da36593f8a8c80e59c65f84ce1d0d4629a1ca8773bb2035200d9483f40";

const isWindows = process.platform === "win32";
const SOURCE_COMMIT = isWindows ? WINDOWS_SOURCE_COMMIT : LINUX_SOURCE_COMMIT;
const ARCHIVE_PATH = isWindows ? WINDOWS_ARCHIVE_PATH : LINUX_ARCHIVE_PATH;
export const ARCADIA_ARCHIVE_SHA256 = isWindows ? WINDOWS_ARCHIVE_SHA256 : LINUX_ARCHIVE_SHA256;

export const ARCADIA_EFFECTIVE_GRANTS = [
  {
    type: "com.kosmos.game",
    version: "1.0.0",
    roles: ["read", "edit"],
    fields_read: [
      "props.description",
      "props.extensions",
      "props.genres",
      "props.platforms",
      "props.playStatus",
      "props.released",
      "props.userRating",
      "title",
    ],
    fields_write: [
      "props.description",
      "props.extensions",
      "props.genres",
      "props.platforms",
      "props.playStatus",
      "props.released",
      "props.userRating",
      "title",
    ],
    relations_read: ["background-image", "cover-image", "note", "tag", "task"],
    relations_write: [],
  },
];
export type ArcadiaInstalledPackage = {
  id: string;
  version: string;
  enabled: boolean;
  effective_grants: typeof ARCADIA_EFFECTIVE_GRANTS;
};
const REQUIRED_ENTRIES = [
  "manifest.json",
  "provenance.json",
  "compatibility.json",
  "icon.png",
  "dist/index.html",
];

type Manifest = { id: string; version: string; icon?: string; [key: string]: JsonValue };
type JsonValue =
  | string
  | number
  | boolean
  | null
  | readonly JsonValue[]
  | { readonly [key: string]: JsonValue };
type Provenance = {
  schema_version: number;
  source_commit: string;
  files: Array<{ path: string; size: number; sha256: string }>;
};

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

export function arcadiaArchive(root: string, repositoryRoot: string) {
  // Reviewed sources: Windows 2689da940071ed5068226a19b28a09490b059dae,
  // Linux ae10decb1aa91e0704302a26bb19df031396bd79.
  const sourceRoot = path.join(repositoryRoot, "arcadia");
  const checkout = execFileSync(gitExecutable, ["rev-parse", "HEAD"], {
    cwd: sourceRoot,
    encoding: "utf8",
    stdio: "pipe",
  }).trim();
  const expectedCheckout = isWindows ? WINDOWS_ARCHIVE_COMMIT : LINUX_SOURCE_COMMIT;
  if (checkout !== expectedCheckout)
    throw new Error(
      `Arcadia checkout ${checkout} does not match reviewed source ${expectedCheckout}`,
    );
  if (isWindows) {
    for (const [name, commit] of [
      ["imago", IMAGO_COMMIT],
      ["arca-sdk", ARCA_SDK_COMMIT],
    ] as const) {
      // The digest-bound archive is the build input; sibling HEADs may advance independently.
      execFileSync(gitExecutable, ["cat-file", "-e", `${commit}^{commit}`], {
        cwd: path.join(repositoryRoot, name),
        stdio: "pipe",
      });
    }
  }

  const file = path.join(root, path.basename(ARCHIVE_PATH));
  const source = path.join(sourceRoot, ARCHIVE_PATH);
  if (!fs.existsSync(source))
    throw new Error(
      `Arcadia release archive not found: ${source} (run \`bun run package:kspkg\` in the Arcadia checkout)`,
    );
  fs.copyFileSync(source, file);
  if (digest(fs.readFileSync(file)) !== ARCADIA_ARCHIVE_SHA256)
    throw new Error("Arcadia release archive digest does not match the reviewed artifact");

  const zip = readZip(file);
  const entries = zip.map((entry) => entry.name);
  if (entries.filter((entry) => entry === "manifest.json").length !== 1)
    throw new Error("Arcadia archive must contain exactly one manifest.json");
  if (entries.some(isUnsafeEntry)) throw new Error("Arcadia archive contains an unsafe path");
  if (new Set(entries.map((entry) => entry.toLowerCase())).size !== entries.length)
    throw new Error("Arcadia archive contains duplicate paths");
  for (const required of REQUIRED_ENTRIES)
    if (!entries.includes(required)) throw new Error(`Arcadia archive is missing ${required}`);

  const zipEntry = (name: string) => {
    const entry = zip.find((candidate) => candidate.name === name);
    if (!entry) throw new Error(`Arcadia archive is missing ${name}`);
    return entry.data;
  };
  // SAFETY: the signed archive contains exactly one validated manifest object.
  const archived = JSON.parse(zipEntry("manifest.json").toString("utf8")) as Manifest;
  // SAFETY: package.manifest.json at the pinned Git commit is a reviewed manifest object.
  const reviewed = JSON.parse(
    execFileSync(gitExecutable, ["show", `${SOURCE_COMMIT}:package.manifest.json`], {
      cwd: sourceRoot,
      encoding: "utf8",
      stdio: "pipe",
    }),
  ) as Manifest;
  if (!isDeepStrictEqual(archived, reviewed))
    throw new Error(`Arcadia archive manifest does not match source commit ${SOURCE_COMMIT}`);
  const workerEntrypoint = (
    archived.targets as readonly { runtime?: string; entrypoint?: string }[]
  ).find((target) => target.runtime === "worker")?.entrypoint;
  if (workerEntrypoint && !entries.includes(workerEntrypoint))
    throw new Error(`Arcadia archive is missing declared worker entrypoint ${workerEntrypoint}`);
  // SAFETY: provenance is compared field-for-field with the archive-derived BOM below.
  const provenance = JSON.parse(zipEntry("provenance.json").toString("utf8")) as Provenance;
  const files = zip
    .filter((entry) => !entry.name.endsWith("/") && entry.name !== "provenance.json")
    .map((entry) => ({
      path: entry.name,
      size: entry.data.length,
      sha256: digest(entry.data),
    }))
    .sort((left, right) => Buffer.compare(Buffer.from(left.path), Buffer.from(right.path)));
  if (
    provenance.schema_version !== 1 ||
    provenance.source_commit !== SOURCE_COMMIT ||
    !isDeepStrictEqual(provenance.files, files)
  )
    throw new Error("Arcadia archive provenance does not match its pinned source and contents");
  return { file, manifest: archived };
}
