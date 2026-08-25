#!/usr/bin/env node
import {
  readJson,
  writeAtomic,
  bytes,
  fail,
  integer,
  iso,
  safeId,
  finish,
} from "./package-release-utils.mjs";

function validate(input) {
  if (input.schema_version !== 1) fail("schema_version must be 1");
  if (
    Object.prototype.toString.call(input.old_key_id) !== "[object String]" ||
    !input.old_key_id ||
    !input.new_key ||
    input.new_key === null || Object.prototype.toString.call(input.new_key) !== "[object Object]"
  )
    fail("old_key_id and new_key are required");
  if (Object.prototype.toString.call(input.new_key.key_id) !== "[object String]" || Object.prototype.toString.call(input.new_key.public_key) !== "[object String]")
    fail("new_key.key_id and new_key.public_key are required");
  safeId(input.old_key_id, "old_key_id");
  safeId(input.new_key.key_id, "new_key.key_id");
  if (input.old_key_id === input.new_key.key_id) fail("old_key_id and new key_id must differ");
  const issuedAt = iso(input.issued_at, "issued_at");
  const expiresAt = iso(input.expires_at, "expires_at");
  if (Date.parse(expiresAt) <= Date.parse(issuedAt)) fail("expires_at must be after issued_at");
  if (integer(input.sequence, "sequence") === 0) fail("sequence must be greater than zero");
  if (!/^[A-Za-z0-9+/]{43}=$/.test(input.new_key.public_key))
    fail("new_key.public_key must be base64 Ed25519 public key");
  return {
    schema_version: 1,
    sequence: integer(input.sequence, "sequence"),
    issued_at: issuedAt,
    expires_at: expiresAt,
    old_key_id: input.old_key_id,
    new_key: { key_id: input.new_key.key_id, public_key: input.new_key.public_key },
  };
}
try {
  const { requireArgs } = await import("./package-release-utils.mjs");
  const args = requireArgs(process.argv, ["input", "output"]);
  await writeAtomic(args.output, bytes(validate(await readJson(args.input))));
} catch (error) {
  finish(error);
}
