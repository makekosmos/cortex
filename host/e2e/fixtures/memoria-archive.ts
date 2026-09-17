import { execFileSync } from "node:child_process";
import { createHash } from "node:crypto";
import fs from "node:fs";
import path from "node:path";
import type { JsonValue, Manifest, PackageArchive } from "./signed-app-types";
import { readZip } from "../../../desktop/scripts/zip-utils.mjs";

const SOURCE_COMMIT = "9ae7892bb6d66615506f2109c12f8438f6f1aa36";
const ARCHIVE_PATH = "release/memoria-0.6.8.kspkg";
const ARCHIVE_SHA256 = "351103b14c75cc0c670e574024ebfd5ec0a56643809e5fc0815cb2260f37e294";

const gitExecutable =
  process.platform === "win32"
    ? path.join(process.env.ProgramFiles ?? "C:\\Program Files", "Git", "cmd", "git.exe")
    : "git";
const digest = (value: string | Buffer) => createHash("sha256").update(value).digest("hex");
const isJsonObject = (value: JsonValue): value is { readonly [key: string]: JsonValue } =>
  typeof value === "object" && value !== null && !Array.isArray(value);

// Package v1 archives are plain ZIPs; the shared desktop reader is portable,
// unlike `tar` (bsdtar on Windows auto-detects ZIP, GNU tar on Linux does not).
const zipEntry = (file: string, name: string) => readZip(file).find((entry) => entry.name === name);

export function memoriaArchive(root: string, repositoryRoot: string): PackageArchive {
  const memoriaRoot = path.join(repositoryRoot, "memoria");
  const file = path.join(root, path.basename(ARCHIVE_PATH));
  let archive: Buffer;
  try {
    archive = execFileSync(
      gitExecutable,
      ["-C", fs.realpathSync.native(memoriaRoot), "show", `${SOURCE_COMMIT}:${ARCHIVE_PATH}`],
      {
        cwd: repositoryRoot,
        encoding: null,
        maxBuffer: 128 * 1024 * 1024,
        stdio: "pipe",
      },
    );
  } catch (error) {
    throw new Error(`Cannot read the pinned Memoria package fixture: ${String(error)}`);
  }
  if (digest(archive) !== ARCHIVE_SHA256)
    throw new Error("Memoria package fixture digest mismatch");
  fs.writeFileSync(file, archive);

  const entries = readZip(file).map((entry) => entry.name);
  if (entries.filter((entry) => entry === "manifest.json").length !== 1) {
    throw new Error("Memoria archive must contain exactly one manifest.json");
  }
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
      throw new Error(`Memoria archive contains an unsafe path: ${entry}`);
    }
  }
  if (new Set(entries.map((entry) => entry.toLowerCase())).size !== entries.length) {
    throw new Error("Memoria archive contains duplicate paths");
  }
  for (const required of ["manifest.json", "compatibility.json", "icon.png", "dist/index.html"]) {
    if (!entries.includes(required)) throw new Error(`Memoria archive is missing ${required}`);
  }
  for (const entry of entries) {
    if (
      entry !== "manifest.json" &&
      entry !== "compatibility.json" &&
      entry !== "icon.png" &&
      entry !== "dist/" &&
      !entry.startsWith("dist/")
    ) {
      throw new Error(`Memoria archive has unexpected entry: ${entry}`);
    }
  }

  let parsed: JsonValue;
  try {
    const manifestEntry = zipEntry(file, "manifest.json");
    if (!manifestEntry) throw new Error("manifest.json entry missing");
    // SAFETY: isJsonObject and the required manifest fields are checked below.
    parsed = JSON.parse(manifestEntry.data.toString("utf8")) as JsonValue;
  } catch (error) {
    throw new Error(`Memoria archive manifest is not valid JSON: ${String(error)}`);
  }
  if (!isJsonObject(parsed)) throw new Error("Memoria archive manifest must be a JSON object");
  if (
    digest(JSON.stringify(parsed)) !==
      "7bdc84375e069b1ba6ba2c99780bcd18c9e5c4d9db2b3285d1508498c96c2b69" ||
    parsed.schema_version !== 2 ||
    parsed.id !== "com.kosmos.memoria" ||
    parsed.version !== "0.6.8" ||
    parsed.kind !== "app" ||
    parsed.icon !== "icon.png" ||
    parsed.entrypoint !== "dist/index.html"
  ) {
    throw new Error("Memoria archive manifest has unexpected identity or entrypoint");
  }
  const compatibilityEntry = zipEntry(file, "compatibility.json");
  if (!compatibilityEntry) throw new Error("Memoria archive is missing compatibility.json");
  if (
    digest(JSON.stringify(JSON.parse(compatibilityEntry.data.toString("utf8")))) !==
    "be1e21bd940532a69c22e026b220a05e3341e4a5c9dfeccd8064e8540171bc61"
  ) {
    throw new Error("Memoria archive has unexpected compatibility metadata");
  }
  const manifest = {
    ...parsed,
    id: parsed.id,
    version: parsed.version,
  } satisfies Manifest;
  return { file, manifest };
}
