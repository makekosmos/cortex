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
  if (!Array.isArray(input.revoked_release_keys) || !Array.isArray(input.revoked_packages))
    fail("revocation lists are required");
  const keys = [...input.revoked_release_keys].sort();
  if (keys.some((key) => typeof key !== "string" || !key)) fail("invalid revoked release key id");
  keys.forEach((key) => safeId(key, "revoked release key id"));
  if (new Set(keys).size !== keys.length) fail("duplicate revoked release key id");
  const packages = input.revoked_packages
    .map((item) => {
      if (!item || typeof item.id !== "string" || typeof item.version !== "string")
        fail("revoked package id/version are required");
      safeId(item.id, "revoked package id");
      semver(item.version, "revoked package version");
      return {
        id: item.id,
        version: item.version,
        sha256: sha256(item.sha256, "revoked package sha256"),
      };
    })
    .sort((a, b) =>
      `${a.id}\0${a.version}\0${a.sha256}`.localeCompare(`${b.id}\0${b.version}\0${b.sha256}`),
    );
  if (integer(input.sequence, "sequence") === 0) fail("sequence must be greater than zero");
  if (packages.length === 0 && keys.length === 0) fail("revocation list must be non-empty");
  if (
    new Set(packages.map((item) => `${item.id}\0${item.version}\0${item.sha256}`)).size !==
    packages.length
  )
    fail("duplicate revoked package tuple");
  return {
    schema_version: 1,
    sequence: integer(input.sequence, "sequence"),
    issued_at: iso(input.issued_at, "issued_at"),
    revoked_release_keys: keys,
    revoked_packages: packages,
  };
}
try {
  const { requireArgs } = await import("./package-release-utils.mjs");
  const args = requireArgs(process.argv, ["input", "output"]);
  await writeAtomic(args.output, bytes(validate(await readJson(args.input))));
} catch (error) {
  finish(error);
}
