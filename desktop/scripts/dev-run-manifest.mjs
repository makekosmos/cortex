import { randomUUID } from "node:crypto";
import {
  existsSync,
  lstatSync,
  mkdirSync,
  readFileSync,
  realpathSync,
  renameSync,
  rmSync,
  writeFileSync,
} from "node:fs";
import path from "node:path";
import { releasePortLease } from "./dev-run-port.mjs";

const within = (candidate, root) => {
  const relative = path.relative(path.resolve(root), path.resolve(candidate));
  return (
    relative !== "" &&
    relative !== ".." &&
    !relative.startsWith(`..${path.sep}`) &&
    !path.isAbsolute(relative)
  );
};
const atomicWrite = (file, value) => {
  mkdirSync(path.dirname(file), { recursive: true });
  const temp = `${file}.${process.pid}.${randomUUID()}.tmp`;
  writeFileSync(temp, `${JSON.stringify(value, null, 2)}\n`, { encoding: "utf8", flag: "wx" });
  renameSync(temp, file);
};

export function createRunManifest(app, root) {
  const nonce = randomUUID();
  const testRoot = path.resolve(root);
  const runId = `${new Date().toISOString().replace(/\D/g, "").slice(0, 14)}-${nonce.slice(0, 8)}`;
  const runRoot = path.join(testRoot, runId);
  return {
    schemaVersion: 1,
    app,
    runId,
    runRoot,
    testRoot,
    dataDir: path.join(runRoot, "data"),
    userDataDir: path.join(runRoot, "data", "userdata"),
    outputDir: path.join(runRoot, "output"),
    ports: { shell: 10000 + (Number.parseInt(nonce.slice(-4), 16) % 50000) },
    portLease: null,
    ownedPids: [],
    createdAt: new Date().toISOString(),
  };
}
export function writeRunManifest(file, manifest) {
  atomicWrite(file, manifest);
}
export function readRunManifest(file, expectedRoot) {
  const manifest = JSON.parse(readFileSync(file, "utf8"));
  if (
    manifest?.schemaVersion !== 1 ||
    Object.prototype.toString.call(manifest.runId) !== "[object String]" ||
    !/^\d{14}-[a-f0-9]{8}$/.test(manifest.runId)
  )
    throw new Error("invalid dev run manifest");
  const root = path.resolve(expectedRoot);
  if (path.resolve(String(manifest.testRoot ?? "")) !== root)
    throw new Error("manifest test root is not canonical");
  const canonical = {
    runRoot: path.join(root, manifest.runId),
    dataDir: path.join(root, manifest.runId, "data"),
    userDataDir: path.join(root, manifest.runId, "data", "userdata"),
    outputDir: path.join(root, manifest.runId, "output"),
  };
  if (!within(file, root) || path.resolve(file) !== path.join(canonical.runRoot, "manifest.json"))
    throw new Error("manifest file is outside test root");
  for (const name of Object.keys(canonical))
    if (manifest[name] !== canonical[name])
      throw new Error(`manifest path ${name} is not canonical`);
  if (!Number.isSafeInteger(manifest.ports?.shell) || manifest.ports.shell < 1)
    throw new Error("manifest shell port is invalid");
  if (
    manifest.portLease !== null &&
    manifest.portLease !== path.join(root, "leases", `${manifest.ports.shell}.lease`)
  )
    throw new Error("manifest port lease is not canonical");
  if (
    !Array.isArray(manifest.ownedPids) ||
    manifest.ownedPids.some(
      (owned) =>
        !Number.isSafeInteger(owned?.pid) ||
        owned.pid < 1 ||
        Object.prototype.toString.call(owned.startTime) !== "[object String]" ||
        Object.prototype.toString.call(owned.commandLine) !== "[object String]" ||
        !owned.commandLine.trim(),
    )
  )
    throw new Error("manifest ownedPids is invalid");
  rejectReparsePath(canonical.runRoot, root);
  rejectReparsePath(file, root);
  return manifest;
}
function rejectReparsePath(candidate, root) {
  let current = path.resolve(candidate);
  const boundary = path.resolve(root);
  while (current === boundary || within(current, boundary)) {
    if (existsSync(current)) {
      if (lstatSync(current).isSymbolicLink()) throw new Error("reset path is outside test root");
      if (path.resolve(realpathSync.native(current)) !== current)
        throw new Error("reset path is outside test root");
    }
    if (current === boundary) return;
    current = path.dirname(current);
  }
  throw new Error("reset path is outside test root");
}
export function resetRun(file, expectedRoot) {
  const manifest = readRunManifest(file, expectedRoot);
  rejectReparsePath(manifest.runRoot, manifest.testRoot);
  rejectReparsePath(file, manifest.testRoot);
  releasePortLease(manifest.portLease, manifest.testRoot);
  rmSync(manifest.runRoot, { recursive: true, force: true });
  return manifest.runRoot;
}
