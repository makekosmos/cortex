#!/usr/bin/env node
import { promises as fs } from "node:fs";
import { createPrivateKey, sign } from "node:crypto";
import { writeAtomic, finish, safeId } from "./package-release-utils.mjs";

function args(argv) {
  const out = {};
  const signers = [];
  for (let i = 2; i < argv.length; i += 1) {
    const flag = argv[i];
    if (flag === "--signer") {
      const value = argv[++i];
      if (!value || !value.includes("="))
        throw new Error("--signer must be key-id=private-key-path");
      signers.push(value);
    } else if (flag.startsWith("--")) {
      const key = flag.slice(2);
      if (i + 1 >= argv.length || argv[i + 1].startsWith("--"))
        throw new Error(`missing value for ${flag}`);
      out[key] = argv[++i];
    } else throw new Error(`unexpected argument ${flag}`);
  }
  if (!out.input || !out.output) throw new Error("required arguments --input and --output");
  if (out.key || out["key-id"]) {
    if (!out.key || !out["key-id"]) throw new Error("--key and --key-id must be provided together");
    signers.push(`${out["key-id"]}=${out.key}`);
  }
  if (signers.length === 0) throw new Error("at least one signer is required");
  const parsed = signers.map((value) => {
    const split = value.indexOf("=");
    const keyId = value.slice(0, split);
    const file = value.slice(split + 1);
    safeId(keyId, "key_id");
    if (!file) throw new Error("private key path is required");
    return { keyId, file };
  });
  if (new Set(parsed.map((item) => item.keyId)).size !== parsed.length)
    throw new Error("duplicate key_id");
  return { input: out.input, output: out.output, signers: parsed };
}

try {
  const options = args(process.argv);
  const payload = await fs.readFile(options.input);
  const signatures = [];
  for (const signer of options.signers) {
    let privateKey;
    try {
      privateKey = createPrivateKey(await fs.readFile(signer.file));
    } catch {
      throw new Error("failed to read signing key");
    }
    if (privateKey.asymmetricKeyType !== "ed25519") throw new Error("signing key must be Ed25519");
    signatures.push({
      key_id: signer.keyId,
      algorithm: "ed25519",
      signature: sign(null, payload, privateKey).toString("base64"),
    });
  }
  signatures.sort((a, b) => a.key_id.localeCompare(b.key_id));
  await writeAtomic(
    options.output,
    Buffer.from(`${JSON.stringify({ schema_version: 1, signatures })}\n`, "utf8"),
  );
} catch (error) {
  finish(error);
}
