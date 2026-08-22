#!/usr/bin/env node
import { execFileSync } from "node:child_process";
import { promises as fs } from "node:fs";
import path from "node:path";
import { fileURLToPath } from "node:url";
import { entriesFromDir, writeZip } from "./zip-utils.mjs";
import {
  bytes,
  readJson,
  sha256 as validateSha256,
  writeAtomic,
} from "./package-release-utils.mjs";

const scriptDir = path.dirname(fileURLToPath(import.meta.url));
function args(argv) {
  const out = { signers: [] };
  for (let i = 2; i < argv.length; i += 1) {
    const flag = argv[i];
    if (flag === "--signer") {
      const value = argv[++i];
      if (!value || !value.includes("="))
        throw new Error("--signer must be key-id=private-key-path");
      out.signers.push(value);
      continue;
    }
    if (!flag.startsWith("--") || i + 1 >= argv.length || argv[i + 1].startsWith("--"))
      throw new Error(`missing value for ${flag}`);
    out[flag.slice(2)] = argv[++i];
  }
  for (const name of ["source", "manifest", "archive", "catalog", "signatures", "archive-url"])
    if (!out[name]) throw new Error(`required argument --${name}`);
  out.sequence ??= process.env.KOSMOS_PACKAGE_SEQUENCE;
  out["issued-at"] ??= process.env.KOSMOS_PACKAGE_ISSUED_AT;
  out["expires-at"] ??= process.env.KOSMOS_PACKAGE_EXPIRES_AT;
  for (const name of ["sequence", "issued-at", "expires-at"])
    if (!out[name]) throw new Error(`required argument --${name}`);
  if (out.signers.length === 0 && process.env.KOSMOS_PACKAGE_SIGNER)
    out.signers = [process.env.KOSMOS_PACKAGE_SIGNER];
  if (out.signers.length === 0) throw new Error("configured package signing key is required");
  return out;
}
function releaseMetadata(options) {
  const sequence = Number(options.sequence);
  if (!Number.isSafeInteger(sequence) || sequence <= 0)
    throw new Error("sequence must be a positive integer");
  const issuedAt = options["issued-at"];
  const expiresAt = options["expires-at"];
  if (
    typeof issuedAt !== "string" ||
    !issuedAt.endsWith("Z") ||
    Number.isNaN(Date.parse(issuedAt)) ||
    typeof expiresAt !== "string" ||
    !expiresAt.endsWith("Z") ||
    Number.isNaN(Date.parse(expiresAt))
  )
    throw new Error("issued-at and expires-at must be ISO UTC timestamps");
  const lifetime = Date.parse(expiresAt) - Date.parse(issuedAt);
  if (lifetime <= 0 || lifetime > 366 * 24 * 60 * 60 * 1000)
    throw new Error("expires-at must be within 366 days after issued-at");
  return { sequence, issuedAt, expiresAt };
}
function validateManifest(manifest, source) {
  if (
    manifest.schema_version !== 2 ||
    !["app", "source", "bridge"].includes(manifest.kind) ||
    manifest.publisher !== "kosmos"
  )
    throw new Error("package manifest must be a compiled Manifest v2 package");
  if (typeof manifest.entrypoint !== "string" || manifest.entrypoint.startsWith("/"))
    throw new Error("package entrypoint must be relative");
  const relativeEntrypoint =
    path.basename(source).toLowerCase() === "dist" && manifest.entrypoint.startsWith("dist/")
      ? manifest.entrypoint.slice("dist/".length)
      : manifest.entrypoint;
  const entrypoint = path.resolve(source, relativeEntrypoint.replaceAll("/", path.sep));
  const sourceRoot = path.resolve(source);
  if (!entrypoint.startsWith(`${sourceRoot}${path.sep}`))
    throw new Error("package entrypoint escapes source");
  if (manifest.icon !== undefined) {
    const icon = manifest.icon;
    if (
      typeof icon !== "string" ||
      icon.includes("\\") ||
      icon.includes("\0") ||
      path.posix.isAbsolute(icon) ||
      path.posix.normalize(icon) !== icon ||
      !/\.(?:ico|png)$/iu.test(icon)
    )
      throw new Error("package icon must be a relative .ico or .png asset path");
  }
  return entrypoint;
}
try {
  const options = args(process.argv);
  const source = path.resolve(options.source);
  const manifest = await readJson(options.manifest);
  const metadata = releaseMetadata(options);
  const entrypoint = validateManifest(manifest, source);
  await fs.access(entrypoint);
  if (manifest.icon && !options.icon)
    throw new Error("--icon is required when manifest declares icon");
  if (!manifest.icon && options.icon) throw new Error("manifest icon is required with --icon");
  const archiveEntries = [
    { name: "manifest.json", data: bytes(manifest) },
    ...(manifest.icon
      ? [{ name: manifest.icon, data: await fs.readFile(path.resolve(options.icon)) }]
      : []),
    ...entriesFromDir(source).map((entry) => ({
      name: `dist/${entry.name}`,
      data: entry.data,
    })),
  ].sort((left, right) => left.name.localeCompare(right.name));
  const archive = path.resolve(options.archive);
  await fs.mkdir(path.dirname(archive), { recursive: true });
  writeZip(archive, archiveEntries);
  const archiveBytes = await fs.readFile(archive);
  const archiveUrl = options["archive-url"]
    .replaceAll("{id}", manifest.id)
    .replaceAll("{version}", manifest.version);
  const catalog = {
    schema_version: 1,
    sequence: metadata.sequence,
    issued_at: metadata.issuedAt,
    expires_at: metadata.expiresAt,
    packages: [
      {
        manifest,
        archive_url: archiveUrl,
        sha256: (await import("node:crypto"))
          .createHash("sha256")
          .update(archiveBytes)
          .digest("hex"),
        size: archiveBytes.length,
      },
    ],
  };
  const catalogPath = path.resolve(options.catalog);
  const rawCatalogPath = `${catalogPath}.${process.pid}.input`;
  await writeAtomic(rawCatalogPath, bytes(catalog));
  try {
    execFileSync(
      process.execPath,
      [
        path.join(scriptDir, "package-catalog.mjs"),
        "--input",
        rawCatalogPath,
        "--output",
        catalogPath,
      ],
      { stdio: "inherit", windowsHide: true },
    );
  } finally {
    await fs.rm(rawCatalogPath, { force: true });
  }
  await fs.mkdir(path.dirname(path.resolve(options.signatures)), {
    recursive: true,
  });
  const signerArgs = options.signers.flatMap((signer) => ["--signer", signer]);
  execFileSync(
    process.execPath,
    [
      path.join(scriptDir, "package-sign.mjs"),
      "--input",
      catalogPath,
      "--output",
      path.resolve(options.signatures),
      ...signerArgs,
    ],
    { stdio: "inherit", windowsHide: true },
  );
  const envelope = path.join(path.dirname(catalogPath), "catalog.envelope.json");
  execFileSync(
    process.execPath,
    [
      path.join(scriptDir, "package-envelope.mjs"),
      "--catalog",
      catalogPath,
      "--signatures",
      path.resolve(options.signatures),
      "--output",
      envelope,
    ],
    { stdio: "inherit", windowsHide: true },
  );
  validateSha256(catalog.packages[0].sha256, "archive sha256");
  console.log(`[package-app] wrote ${archive}, signed ${catalogPath}, and enveloped ${envelope}`);
} catch (error) {
  console.error(`[package-app] ${error instanceof Error ? error.message : String(error)}`);
  process.exitCode = 1;
}
