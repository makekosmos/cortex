// prepare: verify inputs, install the bundled Engine, pin artifact hashes.
import fs from "node:fs";
import path from "node:path";
import { fileURLToPath } from "node:url";
import { installEngineArchive, verifyEngineArchive } from "../engine-distribution.mjs";
import {
  artifactRecord,
  catalogEntries,
  DEFAULT_OUT,
  ensureDir,
  FIXTURE_ENGINE_VERSION,
  gitRevision,
  iso,
  layout,
  readJson,
  REPO,
  writeJson,
} from "./lib.mjs";

// Capabilities required by the production catalog (com.kosmos.shell) that
// released Engine binaries prior to this workspace's runtime do not know.
const CATALOG_REQUIRED_CAPS = ["launcher.search", "notifications"];
const ENGINE_OVERRIDES = {
  "engine-backend": "kepler-backend.exe",
  "engine-ark": "ark-core-rpc.exe",
  "engine-focus-helper": "kepler-focus-helper.exe",
  "engine-focus-svc": "kepler-focus-svc.exe",
};

export async function cmdPrepare(args) {
  const outDir = path.resolve(args.out ?? DEFAULT_OUT);
  const paths = layout(outDir);
  const packagedRoot = args["packaged-root"] ? path.resolve(args["packaged-root"]) : null;
  const problems = [];
  const require = (file, label) => {
    if (!file || !fs.existsSync(file)) {
      problems.push(`${label} missing: ${file ?? "(not provided)"}`);
      return null;
    }
    return path.resolve(file);
  };
  const shellExe = packagedRoot && require(path.join(packagedRoot, "Kosmos.exe"), "Kosmos.exe");
  const hostExe =
    packagedRoot &&
    require(path.join(
      packagedRoot,
      "resources",
      "components",
      "host",
      "Kosmos Package Host.exe",
    ), "packaged Host");
  const engineZip =
    packagedRoot &&
    require(path.join(packagedRoot, "resources", "Kosmos Engine.zip"), "Kosmos Engine.zip");
  const engineManifestFile =
    packagedRoot &&
    require(path.join(packagedRoot, "resources", "engine-manifest.json"), "engine-manifest.json");
  const catalogFile = require(args.catalog, "production catalog");
  const agendaKspkg = require(args["agenda-kspkg"], "Agenda .kspkg");
  const memoriaKspkg = require(args["memoria-kspkg"], "Memoria .kspkg");
  const engineOverrides = Object.entries(ENGINE_OVERRIDES)
    .map(([flag, name]) => [name, args[flag] ? path.resolve(args[flag]) : null])
    .filter(([, file]) => file !== null);
  for (const [name, file] of engineOverrides)
    if (!fs.existsSync(file)) problems.push(`Engine override ${name} missing: ${file}`);
  if (problems.length) {
    console.error(`prepare: missing inputs:\n  ${problems.join("\n  ")}`);
    process.exitCode = 2;
    return;
  }

  // Verify and install the bundled Engine exactly like the packaged smoke does.
  const engineManifest = readJson(engineManifestFile);
  verifyEngineArchive(engineZip, engineManifest);
  let versionRoot = installEngineArchive(engineZip, engineManifest, paths.engineRoot);
  let engineVersion = engineManifest.version;
  let engineProvenance = { mode: "packaged", version: engineVersion };

  // Optional binary overrides stage a fixture-local Engine version so the
  // packaged archive stays pristine and every artifact hash is recorded.
  if (engineOverrides.length) {
    const overrideRoot = path.join(paths.engineRoot, "versions", FIXTURE_ENGINE_VERSION);
    fs.rmSync(overrideRoot, { recursive: true, force: true });
    ensureDir(overrideRoot);
    for (const entry of fs.readdirSync(versionRoot)) {
      const source = path.join(versionRoot, entry);
      if (fs.statSync(source).isFile()) fs.copyFileSync(source, path.join(overrideRoot, entry));
    }
    const records = {};
    for (const [name, file] of engineOverrides) {
      fs.copyFileSync(file, path.join(overrideRoot, name));
      records[name] = artifactRecord(file, { source: file });
    }
    const files = fs
      .readdirSync(overrideRoot)
      .filter((name) => name !== "engine-manifest.json")
      .map((name) => {
        const record = artifactRecord(path.join(overrideRoot, name));
        return { name, sha256: record.sha256, size: record.size };
      });
    writeJson(path.join(overrideRoot, "engine-manifest.json"), {
      schema_version: 1,
      product: "kosmos-engine",
      version: FIXTURE_ENGINE_VERSION,
      source_commit: gitRevision().sha ?? "0".repeat(40),
      files,
    });
    fs.writeFileSync(
      path.join(paths.engineRoot, "current.json"),
      JSON.stringify({ schema_version: 1, version: FIXTURE_ENGINE_VERSION }),
      "utf8",
    );
    versionRoot = overrideRoot;
    engineVersion = FIXTURE_ENGINE_VERSION;
    engineProvenance = {
      mode: "override",
      base_version: engineManifest.version,
      staged_version: FIXTURE_ENGINE_VERSION,
      overrides: records,
    };
  }
  const installed = {
    version: engineVersion,
    backend: path.join(versionRoot, "kepler-backend.exe"),
    ark: path.join(versionRoot, "ark-core-rpc.exe"),
  };
  for (const [label, file] of Object.entries(installed)) {
    if (label !== "version" && !fs.existsSync(file))
      throw new Error(`installed Engine artifact missing: ${file}`);
  }
  // The engine archive has no tray.ico; the packaged resources carry it.
  const trayIcon = path.join(packagedRoot, "resources", "tray.ico");

  // Capability probe: the production catalog requires capabilities that
  // released Engine binaries may predate (engine <=0.1.4 lacks
  // launcher.search/notifications, so every published catalog sequence fails
  // manifest validation there). String presence is a heuristic, not a parser.
  const backendBytes = fs.readFileSync(installed.backend).toString("latin1");
  const capabilityProbe = Object.fromEntries(
    [
      "ark.read",
      "ark.write",
      "filesystem.read",
      "filesystem.write",
      "network",
      "clipboard",
      "process.spawn",
      "worker.invoke",
      "dictation.control",
      ...CATALOG_REQUIRED_CAPS,
    ].map((capability) => [capability, backendBytes.includes(capability)]),
  );
  const catalogCompatible = CATALOG_REQUIRED_CAPS.every(
    (capability) => capabilityProbe[capability],
  );

  // Verify packages against the pinned production catalog entries.
  const catalogEnvelope = readJson(catalogFile);
  const { document: catalogDocument, packages } = catalogEntries(catalogEnvelope);
  const catalogById = new Map(packages.map((entry) => [entry.manifest?.id ?? entry.id, entry]));
  const packagesFixed = {};
  for (const [key, file] of [
    ["com.kosmos.agenda", agendaKspkg],
    ["com.kosmos.memoria", memoriaKspkg],
  ]) {
    const entry = catalogById.get(key);
    if (!entry) throw new Error(`catalog has no entry for ${key}`);
    const artifact = artifactRecord(file);
    const expectedSha = String(entry.sha256 ?? "").toLowerCase();
    const expectedSize = Number(entry.size);
    if (artifact.sha256 !== expectedSha)
      throw new Error(
        `${key} archive hash mismatch: file ${artifact.sha256} catalog ${expectedSha}`,
      );
    if (Number.isSafeInteger(expectedSize) && artifact.size !== expectedSize)
      throw new Error(`${key} archive size mismatch: ${artifact.size} != ${expectedSize}`);
    const version = entry.manifest?.version ?? entry.version;
    packagesFixed[key] = { ...artifact, version, catalogEntry: entry };
  }

  ensureDir(paths.inputs);
  fs.copyFileSync(catalogFile, path.join(paths.inputs, "catalog.json"));
  fs.copyFileSync(agendaKspkg, path.join(paths.inputs, path.basename(agendaKspkg)));
  fs.copyFileSync(memoriaKspkg, path.join(paths.inputs, path.basename(memoriaKspkg)));

  const bomFile = path.join(packagedRoot, "resources", "release-bom.json");
  const manifest = {
    schema_version: 1,
    created_at: iso(),
    prepare_command: `node ${path.relative(REPO, fileURLToPath(import.meta.url)).replaceAll("\\", "/")} ${process.argv.slice(2).join(" ")}`,
    git: gitRevision(),
    packaged_root: packagedRoot,
    release_bom: fs.existsSync(bomFile) ? readJson(bomFile) : null,
    artifacts: {
      shell_exe: artifactRecord(shellExe),
      host_exe: artifactRecord(hostExe),
      engine_zip: artifactRecord(engineZip),
      engine_manifest: artifactRecord(engineManifestFile),
      catalog: artifactRecord(catalogFile),
      agenda_kspkg: packagesFixed["com.kosmos.agenda"],
      memoria_kspkg: packagesFixed["com.kosmos.memoria"],
    },
    engine: {
      root: paths.engineRoot,
      version: installed.version,
      backend: installed.backend,
      ark: installed.ark,
      tray: fs.existsSync(trayIcon) ? artifactRecord(trayIcon).path : null,
      manifest: engineManifest,
      provenance: engineProvenance,
      capability_probe: capabilityProbe,
      catalog_compatible: catalogCompatible,
    },
    catalog: {
      sequence: catalogDocument.sequence ?? null,
      issued_at: catalogDocument.issued_at ?? null,
      expires_at: catalogDocument.expires_at ?? null,
    },
    packages: Object.fromEntries(
      Object.entries(packagesFixed).map(([id, info]) => [
        id,
        { version: info.version, sha256: info.sha256, size: info.size, kspkg: info.path },
      ]),
    ),
  };
  writeJson(paths.manifest, manifest);
  console.log(
    JSON.stringify({
      result: "pass",
      out: outDir,
      engine_version: installed.version,
      catalog_sequence: manifest.catalog.sequence,
      packages: manifest.packages,
    }),
  );
}
