#!/usr/bin/env node
import { promises as fs } from "node:fs";
import { bytes, finish, requireArgs, writeAtomic } from "./package-release-utils.mjs";
try {
  const args = requireArgs(process.argv, ["catalog", "signatures", "output"]);
  const catalog = await fs.readFile(args.catalog);
  const signatures = JSON.parse(await fs.readFile(args.signatures, "utf8"));
  if (!signatures || signatures.schema_version !== 1 || !Array.isArray(signatures.signatures))
    throw new Error("invalid signature set");
  await writeAtomic(args.output, bytes({ bytes: catalog.toString("base64"), signatures }));
} catch (error) {
  finish(error);
}
