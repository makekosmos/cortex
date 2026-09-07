import { execFileSync } from "node:child_process";
import { createHash } from "node:crypto";
import fs from "node:fs";
import path from "node:path";
import type { JsonValue, Manifest, PackageArchive } from "./signed-app-types";

const SOURCE_COMMIT = "1d72b61";
const ARCHIVE_PATH = "release/memoria-0.6.6.kspkg";
const ARCHIVE_SHA256 = "9aa76cd10cfb1d8be9f5ca9770407496c9aa9e7691ec3dc0a3bb32b7af88bbaf";

const command = (file: string, args: string[], cwd: string) =>
  execFileSync(file, args, { cwd, encoding: "utf8", stdio: "pipe" });
const gitExecutable =
  process.platform === "win32"
    ? path.join(process.env.ProgramFiles ?? "C:\\Program Files", "Git", "cmd", "git.exe")
    : "git";
const digest = (value: string | Buffer) => createHash("sha256").update(value).digest("hex");
const isJsonObject = (value: JsonValue): value is { readonly [key: string]: JsonValue } =>
  typeof value === "object" && value !== null && !Array.isArray(value);

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

  const entries = command("tar", ["-tf", file], root).split(/\r?\n/).filter(Boolean);
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
    // SAFETY: isJsonObject and the required manifest fields are checked below.
    parsed = JSON.parse(command("tar", ["-xOf", file, "manifest.json"], root)) as JsonValue;
  } catch (error) {
    throw new Error(`Memoria archive manifest is not valid JSON: ${String(error)}`);
  }
  if (!isJsonObject(parsed)) throw new Error("Memoria archive manifest must be a JSON object");
  if (
    digest(JSON.stringify(parsed)) !==
      "fbcaba86002fd31f8d9ceccdeecb17bed048881ef30f4c6d930fffc8579b864b" ||
    parsed.schema_version !== 2 ||
    parsed.id !== "com.kosmos.memoria" ||
    parsed.version !== "0.6.6" ||
    parsed.kind !== "app" ||
    parsed.icon !== "icon.png" ||
    parsed.entrypoint !== "dist/index.html"
  ) {
    throw new Error("Memoria archive manifest has unexpected identity or entrypoint");
  }
  const compatibility = command("tar", ["-xOf", file, "compatibility.json"], root);
  if (
    digest(JSON.stringify(JSON.parse(compatibility))) !==
    "4f5dc7b1d4423217fcb37a7c02f71b4d23b2ce07629dd57eaa95f33c663e3c31"
  ) {
    throw new Error("Memoria archive has unexpected compatibility metadata");
  }
  const manifest = { ...parsed, id: parsed.id, version: parsed.version } satisfies Manifest;
  return { file, manifest };
}
