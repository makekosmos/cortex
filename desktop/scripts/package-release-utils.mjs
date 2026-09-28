import { createHash } from "node:crypto";
import { promises as fs } from "node:fs";
import path from "node:path";

export function fail(message) {
  throw new Error(message);
}

export function canonical(value) {
  if (Array.isArray(value)) return value.map(canonical);
  if (value && Object.prototype.toString.call(value) === "[object Object]") {
    return Object.fromEntries(
      Object.keys(value)
        .sort()
        .map((key) => [key, canonical(value[key])]),
    );
  }
  return value;
}

export function bytes(value) {
  return Buffer.from(`${JSON.stringify(canonical(value))}\n`, "utf8");
}

export async function readJson(file) {
  const value = JSON.parse(await fs.readFile(file, "utf8"));
  if (!value || Object.prototype.toString.call(value) !== "[object Object]" || Array.isArray(value))
    fail("input must be a JSON object");
  return value;
}

export async function writeAtomic(file, data) {
  const absolute = path.resolve(file);
  await fs.mkdir(path.dirname(absolute), { recursive: true });
  const temp = `${absolute}.${process.pid}.${Date.now()}.tmp`;
  try {
    await fs.writeFile(temp, data, { flag: "wx", mode: 0o600 });
    await fs.rename(temp, absolute);
  } finally {
    await fs.rm(temp, { force: true });
  }
}

export function requireArgs(argv, names) {
  const args = {};
  for (let i = 2; i < argv.length; i += 1) {
    const key = argv[i];
    if (!key.startsWith("--") || i + 1 >= argv.length || argv[i + 1].startsWith("--"))
      fail(`missing value for ${key}`);
    args[key.slice(2)] = argv[++i];
  }
  for (const name of names) if (!args[name]) fail(`required argument --${name}`);
  return args;
}

export function integer(value, name) {
  const n = Number(value);
  if (!Number.isSafeInteger(n) || n < 0) fail(`${name} must be a non-negative integer`);
  return n;
}

export function iso(value, name) {
  if (
    Object.prototype.toString.call(value) !== "[object String]" ||
    !value.endsWith("Z") ||
    Number.isNaN(Date.parse(value))
  )
    fail(`${name} must be an ISO UTC timestamp`);
  return value;
}

export function safeId(value, name) {
  if (
    Object.prototype.toString.call(value) !== "[object String]" ||
    !/^[a-z0-9](?:[a-z0-9._-]{0,62}[a-z0-9])?$/.test(value)
  )
    fail(`${name} must be a safe identifier`);
  return value;
}

export function semver(value, name) {
  if (
    Object.prototype.toString.call(value) !== "[object String]" ||
    !/^\d+\.\d+\.\d+(?:-[0-9A-Za-z.-]+)?(?:\+[0-9A-Za-z.-]+)?$/.test(value)
  )
    fail(`${name} must be semantic version`);
  return value;
}

export function sha256(value, name) {
  if (Object.prototype.toString.call(value) !== "[object String]" || !/^[a-f0-9]{64}$/.test(value))
    fail(`${name} must be lowercase SHA-256`);
  return value;
}

export function documentHash(data) {
  return createHash("sha256").update(data).digest("hex");
}

export function finish(error) {
  console.error(`[package-release] ${error instanceof Error ? error.message : String(error)}`);
  process.exitCode = 1;
}
