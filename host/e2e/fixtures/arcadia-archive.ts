import { execFileSync } from "node:child_process";
import { createHash } from "node:crypto";
import fs from "node:fs";
import path from "node:path";
import { isDeepStrictEqual } from "node:util";

const DIGEST = "a8b6454bb48518122609fa80f3378fd37fd219160fee525b823f5e624da58332";
const SOURCE_COMMIT = "8774c25d99818fec702bfd513697639b84e7c2c8";
const IMAGO_COMMIT = "b1852cab9f8f08ae0b236b7759138d3117f720b8";
const ARCA_SDK_COMMIT = "21c2f5e157944e3444d3c0604da5a0e041116077";
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
  "worker/arcadia-worker.exe",
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
  // Reviewed source: Arcadia commit 8774c25d99818fec702bfd513697639b84e7c2c8.
  const sourceRoot = path.join(repositoryRoot, "arcadia");
  const source = path.join(sourceRoot, "release", "arcadia-0.1.9.kspkg");
  if (!fs.existsSync(source)) throw new Error(`Arcadia release archive not found: ${source}`);
  const checkout = execFileSync("git", ["rev-parse", "HEAD"], {
    cwd: sourceRoot,
    encoding: "utf8",
    stdio: "pipe",
  }).trim();
  if (checkout !== SOURCE_COMMIT)
    throw new Error(`Arcadia checkout ${checkout} does not match reviewed commit ${SOURCE_COMMIT}`);
  for (const [name, commit] of [
    ["imago", IMAGO_COMMIT],
    ["arca-sdk", ARCA_SDK_COMMIT],
  ] as const) {
    // The digest-bound archive is the build input; sibling HEADs may advance independently.
    execFileSync("git", ["cat-file", "-e", `${commit}^{commit}`], {
      cwd: path.join(repositoryRoot, name),
      stdio: "pipe",
    });
  }

  const file = path.join(root, path.basename(source));
  fs.copyFileSync(source, file);
  if (createHash("sha256").update(fs.readFileSync(file)).digest("hex") !== DIGEST)
    throw new Error("Arcadia release archive digest does not match the reviewed artifact");

  const tar = (...args: string[]) =>
    execFileSync("tar", args, { cwd: root, encoding: "utf8", stdio: "pipe" });
  const entries = tar("-tf", file).split(/\r?\n/).filter(Boolean);
  if (entries.filter((entry) => entry === "manifest.json").length !== 1)
    throw new Error("Arcadia archive must contain exactly one manifest.json");
  if (entries.some(isUnsafeEntry)) throw new Error("Arcadia archive contains an unsafe path");
  if (new Set(entries.map((entry) => entry.toLowerCase())).size !== entries.length)
    throw new Error("Arcadia archive contains duplicate paths");
  for (const required of REQUIRED_ENTRIES)
    if (!entries.includes(required)) throw new Error(`Arcadia archive is missing ${required}`);

  // SAFETY: the signed archive contains exactly one validated manifest object.
  const archived = JSON.parse(tar("-xOf", file, "manifest.json")) as Manifest;
  // SAFETY: package.manifest.json at the pinned Git commit is a reviewed manifest object.
  const reviewed = JSON.parse(
    execFileSync("git", ["show", `${SOURCE_COMMIT}:package.manifest.json`], {
      cwd: sourceRoot,
      encoding: "utf8",
      stdio: "pipe",
    }),
  ) as Manifest;
  if (!isDeepStrictEqual(archived, reviewed))
    throw new Error(`Arcadia archive manifest does not match source commit ${SOURCE_COMMIT}`);
  // SAFETY: provenance is compared field-for-field with the archive-derived BOM below.
  const provenance = JSON.parse(tar("-xOf", file, "provenance.json")) as Provenance;
  const files = entries
    .filter((entry) => !entry.endsWith("/") && entry !== "provenance.json")
    .map((entry) => {
      const data = execFileSync("tar", ["-xOf", file, entry], {
        cwd: root,
        stdio: "pipe",
        maxBuffer: 4 * 1024 * 1024,
      });
      return {
        path: entry,
        size: data.length,
        sha256: createHash("sha256").update(data).digest("hex"),
      };
    })
    .sort((left, right) => Buffer.compare(Buffer.from(left.path), Buffer.from(right.path)));
  if (
    provenance.schema_version !== 1 ||
    provenance.source_commit !== SOURCE_COMMIT ||
    !isDeepStrictEqual(provenance.files, files)
  )
    throw new Error("Arcadia archive provenance does not match its pinned source and contents");
  return { file, manifest: archived };
}
