#!/usr/bin/env node
import {
  readJson,
  writeAtomic,
  bytes,
  fail,
  integer,
  iso,
  sha256,
  safeId,
  semver,
  finish,
} from "./package-release-utils.mjs";

function validate(input) {
  if (input.schema_version !== 1) fail("schema_version must be 1");
  const issuedAt = iso(input.issued_at, "issued_at");
  const expiresAt = iso(input.expires_at, "expires_at");
  if (Date.parse(expiresAt) <= Date.parse(issuedAt)) fail("expires_at must be after issued_at");
  const packages = input.packages;
  const output = {
    schema_version: 1,
    sequence: integer(input.sequence, "sequence"),
    issued_at: issuedAt,
    expires_at: expiresAt,
    packages,
  };
  if (output.sequence === 0) fail("sequence must be greater than zero");
  if (!Array.isArray(packages) || packages.length === 0) fail("packages must be a non-empty array");
  const seen = new Set();
  for (const entry of packages) {
    if (
      !entry ||
      typeof entry !== "object" ||
      !entry.manifest ||
      typeof entry.manifest !== "object"
    )
      fail("entry.manifest is required");
    const manifest = entry.manifest;
    for (const field of [
      "schema_version",
      "id",
      "name",
      "version",
      "kind",
      "engine_api",
      "entrypoint",
      "publisher",
      "permissions",
    ])
      if (!(field in manifest)) fail(`manifest.${field} is required`);
    if (
      manifest.schema_version !== 2 ||
      typeof manifest.name !== "string" ||
      !manifest.name ||
      typeof manifest.engine_api !== "string" ||
      !manifest.engine_api ||
      typeof manifest.entrypoint !== "string" ||
      !manifest.entrypoint ||
      !["app", "source", "bridge"].includes(manifest.kind) ||
      manifest.publisher !== "kosmos" ||
      !Array.isArray(manifest.permissions)
    )
      fail("invalid complete package manifest");
    if (
      !Array.isArray(manifest.targets) ||
      !manifest.targets.length ||
      !manifest.data ||
      typeof manifest.data !== "object" ||
      !Array.isArray(manifest.data.access) ||
      !Array.isArray(manifest.data.defines) ||
      !Array.isArray(manifest.data.mappings)
    )
      fail("invalid v2 package manifest contract");
    safeId(manifest.id, "manifest.id");
    semver(manifest.version, "manifest.version");
    if (typeof entry.archive_url !== "string" || !entry.archive_url.startsWith("https://"))
      fail("archive_url must use HTTPS");
    sha256(entry.sha256, "entry.sha256");
    if (integer(entry.size, "entry.size") === 0) fail("entry.size must be greater than zero");
    const key = `${entry.manifest.id}\0${entry.manifest.version}`;
    if (seen.has(key)) fail("duplicate package id/version");
    seen.add(key);
  }
  return output;
}

try {
  const args = (await import("./package-release-utils.mjs")).requireArgs(process.argv, [
    "input",
    "output",
  ]);
  await writeAtomic(args.output, bytes(validate(await readJson(args.input))));
} catch (error) {
  finish(error);
}
