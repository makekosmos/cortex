import { createHash } from "node:crypto";
import { execFileSync } from "node:child_process";
import fs from "node:fs";
import path from "node:path";
import { isDeepStrictEqual } from "node:util";
import { daedalusArchive } from "./daedalus-archive";
import { dictationArchive } from "./dictation-archive";
import { arcadiaArchive } from "./arcadia-archive";
import { memoriaArchive } from "./memoria-archive";
import { ordoArchive } from "./ordo-archive";
import { TEST_ONLY_RELEASE, TEST_ONLY_ROOT } from "./signing-keys";
import type { JsonValue, Manifest, PackageArchive, Permission } from "./signed-app-types";
import { entriesFromDir, readZip, writeZip } from "../../../desktop/scripts/zip-utils.mjs";
type SignedApps = {
  archives: Record<string, string>;
  versions: Record<string, string>;
  catalog: string;
  signatures: JsonValue;
  trust: { root: string; releases: string };
};
const command = (file: string, args: string[], cwd: string) =>
  execFileSync(file, args, { cwd, encoding: "utf8", stdio: "pipe" });
const cortexRoot = (repositoryRoot: string) => {
  const roots = [repositoryRoot, path.join(repositoryRoot, "cortex")];
  const root = roots.find((candidate) =>
    fs.existsSync(path.join(candidate, "desktop", "scripts", "package-sign.mjs")),
  );
  if (root === undefined)
    throw new Error("Could not locate Cortex desktop/scripts/package-sign.mjs");
  return root;
};
// Portable ZIP write: Package v1 archives are plain ZIPs, and the fixture does
// not require compression — the shared desktop writer is enough on every OS.
const zipDirectory = (stage: string, file: string) => writeZip(file, entriesFromDir(stage));

const archive = (
  root: string,
  id: string,
  permissions: readonly Permission[],
  closeOnLoad = false,
): PackageArchive => {
  const stage = path.join(root, id);
  const file = path.join(root, `${id}.kspkg`);
  const typedArkData = permissions.some((permission) => permission.capability === "ark.write")
    ? {
        access: [
          {
            type: "com.kosmos.note",
            versions: ">=1.0.0",
            actions: ["read", "create", "update", "delete", "subscribe"],
            fields: { read: ["*"], write: ["title", "content", "props.*"] },
            relations: { read: [], write: [] },
          },
        ],
        defines: [],
        mappings: [],
      }
    : { access: [], defines: [], mappings: [] };
  const manifest = {
    schema_version: 2,
    id,
    name: id,
    version: "1.0.0",
    kind: "app",
    engine_api: ">=1.0.0",
    entrypoint: "index.html",
    publisher: "kosmos",
    permissions,
    targets: [{ runtime: "kosmos-host", os: ["windows"] }],
    data: typedArkData,
  };
  fs.mkdirSync(stage, { recursive: true });
  fs.writeFileSync(path.join(stage, "manifest.json"), JSON.stringify(manifest));
  fs.writeFileSync(
    path.join(stage, "index.html"),
    `<title>${id}</title><script src="app.js"></script>`,
  );
  fs.writeFileSync(
    path.join(stage, "app.js"),
    `${closeOnLoad ? "setTimeout(()=>window.kosmosApp.window.close(),0);" : ""}window.__hostEvents=[];window.kosmosApp.ark.subscribe((event)=>window.__hostEvents.push(event));`,
  );
  zipDirectory(stage, file);
  return { file, manifest };
};

const packageDirectory = (
  root: string,
  repositoryRoot: string,
  id: string,
  source: string,
  manifest: Manifest,
) => {
  const stage = path.join(root, id);
  const file = path.join(root, `${id}.kspkg`);
  fs.cpSync(source, path.join(stage, "dist"), { recursive: true });
  fs.writeFileSync(path.join(stage, "manifest.json"), JSON.stringify(manifest));
  if (manifest.icon !== undefined) {
    const productIcon = path.join(path.dirname(source), ...manifest.icon.split("/"));
    const icon = fs.existsSync(productIcon)
      ? productIcon
      : path.join(cortexRoot(repositoryRoot), "desktop", "build", "icon.ico");
    const target = path.join(stage, ...manifest.icon.split("/"));
    fs.mkdirSync(path.dirname(target), { recursive: true });
    fs.copyFileSync(icon, target);
  }
  zipDirectory(stage, file);
  return { file, manifest };
};

const isJsonObject = (value: JsonValue): value is { readonly [key: string]: JsonValue } =>
  typeof value === "object" && value !== null && !Array.isArray(value);

function agendaArchive(root: string, repositoryRoot: string): PackageArchive {
  const agendaRoot = path.join(repositoryRoot, "agenda");
  const agendaCommit = "04425784fda4864e5f66fc575d3d042864a4b8dd";
  const archivePath = "release/agenda-0.2.7.kspkg";
  const file = path.join(root, path.basename(archivePath));
  const archive = execFileSync("git", ["show", `${agendaCommit}:${archivePath}`], {
    cwd: agendaRoot,
    maxBuffer: 128 * 1024 * 1024,
    stdio: "pipe",
  });
  if (
    createHash("sha256").update(archive).digest("hex") !==
    "fb90fd4e6f09c4b99267ff6b636821d6753486f0b6b3f56778846e8f943f59b7"
  ) {
    throw new Error("Agenda package fixture digest mismatch");
  }
  fs.writeFileSync(file, archive);
  const entries = readZip(file).map((entry) => entry.name);
  if (entries.filter((entry) => entry === "manifest.json").length !== 1) {
    throw new Error("Agenda archive must contain exactly one manifest.json");
  }
  for (const entry of entries) {
    const normalized = entry.replace(/\/$/, "");
    const segments = normalized.split("/");
    if (
      entry.includes("\\") ||
      entry.startsWith("/") ||
      /^[A-Za-z]:/.test(entry) ||
      normalized.includes("//") ||
      segments.includes(".") ||
      segments.includes("..")
    )
      throw new Error(`Agenda archive contains an unsafe path: ${entry}`);
  }
  if (new Set(entries.map((entry) => entry.toLowerCase())).size !== entries.length)
    throw new Error("Agenda archive contains duplicate paths");
  for (const required of ["manifest.json", "icon.png", "dist/index.html"])
    if (!entries.includes(required)) throw new Error(`Agenda archive is missing ${required}`);
  for (const entry of entries)
    if (
      entry !== "manifest.json" &&
      entry !== "icon.png" &&
      entry !== "schemas/" &&
      !entry.startsWith("schemas/") &&
      entry !== "dist/" &&
      !entry.startsWith("dist/")
    )
      throw new Error(`Agenda archive has unexpected entry: ${entry}`);
  let parsed: JsonValue;
  try {
    const manifestEntry = readZip(file).find((entry) => entry.name === "manifest.json");
    if (!manifestEntry) throw new Error("manifest.json entry missing");
    // SAFETY: isJsonObject and the required manifest fields are checked below.
    parsed = JSON.parse(manifestEntry.data.toString("utf8")) as JsonValue;
  } catch (error) {
    throw new Error(`Agenda archive manifest is not valid JSON: ${String(error)}`);
  }
  if (!isJsonObject(parsed)) throw new Error("Agenda archive manifest must be a JSON object");
  // SAFETY: the pinned repository manifest is compared deeply with the validated archive object.
  const reviewed = JSON.parse(
    command("git", ["show", `${agendaCommit}:manifest.json`], agendaRoot),
  ) as JsonValue;
  if (
    !isDeepStrictEqual(parsed, reviewed) ||
    parsed.id !== "com.kosmos.agenda" ||
    parsed.version !== "0.2.7" ||
    parsed.entrypoint !== "dist/index.html"
  ) {
    throw new Error("Agenda archive manifest has unexpected id, version, or entrypoint");
  }
  const manifest = { ...parsed, id: parsed.id, version: parsed.version } satisfies Manifest;
  return { file, manifest };
}

const sign = (
  root: string,
  repositoryRoot: string,
  apps: Array<{ file: string; manifest: Manifest }>,
): SignedApps => {
  const catalog = JSON.stringify({
    schema_version: 1,
    sequence: 1,
    issued_at: "2026-07-01T00:00:00Z",
    expires_at: "2030-01-01T00:00:00Z",
    packages: apps.map(({ file, manifest }) => ({
      manifest,
      archive_url: `https://fixture.invalid/${path.basename(file)}`,
      sha256: createHash("sha256").update(fs.readFileSync(file)).digest("hex"),
      size: fs.statSync(file).size,
    })),
  });
  const catalogFile = path.join(root, "catalog.json");
  const keyFile = path.join(root, "release.pem");
  const signatureFile = path.join(root, "catalog.signatures.json");
  fs.writeFileSync(catalogFile, catalog);
  fs.writeFileSync(keyFile, TEST_ONLY_RELEASE.privateKey);
  command(
    process.execPath,
    [
      path.join(cortexRoot(repositoryRoot), "desktop", "scripts", "package-sign.mjs"),
      "--input",
      catalogFile,
      "--output",
      signatureFile,
      "--key-id",
      "release-1",
      "--key",
      keyFile,
    ],
    repositoryRoot,
  );
  return {
    archives: Object.fromEntries(apps.map(({ file, manifest }) => [manifest.id, file])),
    versions: Object.fromEntries(apps.map(({ manifest }) => [manifest.id, manifest.version])),
    catalog,
    signatures: JSON.parse(fs.readFileSync(signatureFile, "utf8")),
    trust: {
      root: JSON.stringify({ key_id: "root", public_key: TEST_ONLY_ROOT.publicKey }),
      releases: JSON.stringify([{ key_id: "release-1", public_key: TEST_ONLY_RELEASE.publicKey }]),
    },
  };
};

function graphArchive(root: string, repositoryRoot: string) {
  const graphRoot = path.join(repositoryRoot, "products", "cosmos-graph");
  const dist = path.join(graphRoot, "dist");
  if (!fs.existsSync(path.join(dist, "index.html")))
    throw new Error("build products/cosmos-graph before its Host E2E");
  // SAFETY: the checked product manifest is generated by the repository fixture.
  const manifest = JSON.parse(
    fs.readFileSync(path.join(graphRoot, "manifest.json"), "utf8"),
  ) as Manifest;
  return packageDirectory(root, repositoryRoot, String(manifest.id), dist, manifest);
}

function shellArchive(root: string, repositoryRoot: string) {
  const shellRoot = path.join(repositoryRoot, "products", "shell");
  const dist = path.join(shellRoot, "dist");
  if (!fs.existsSync(path.join(dist, "index.html")))
    throw new Error("build products/shell before its Host E2E");
  // SAFETY: the checked product manifest is generated by the repository fixture.
  const manifest = JSON.parse(
    fs.readFileSync(path.join(shellRoot, "manifest.json"), "utf8"),
  ) as Manifest;
  return packageDirectory(root, repositoryRoot, String(manifest.id), dist, manifest);
}

export function createSignedApps(
  root: string,
  repositoryRoot: string,
  includeGraph = false,
  includeShell = false,
  closeFixture = false,
  includeAgenda = false,
  includeMemoria = false,
  includeDaedalus = false,
  includeDictation = false,
  includeOrdo = false,
  includeArcadia = false,
): SignedApps {
  const apps: Array<{ file: string; manifest: Manifest }> = [
    archive(
      root,
      "host-e2e-app-a",
      [
        { capability: "ark.write", scopes: ["upsert_object_type", "upsert_object"] },
        { capability: "ark.read", scopes: ["get_object"] },
      ],
      closeFixture,
    ),
    archive(root, "host-e2e-app-b", []),
  ];
  if (includeGraph) apps.push(graphArchive(root, repositoryRoot));
  if (includeShell) apps.push(shellArchive(root, repositoryRoot));
  if (includeAgenda) apps.push(agendaArchive(root, repositoryRoot));
  if (includeMemoria) apps.push(memoriaArchive(root, repositoryRoot));
  if (includeDaedalus) apps.push(daedalusArchive(root, repositoryRoot));
  if (includeDictation) apps.push(dictationArchive(root, repositoryRoot));
  if (includeOrdo) apps.push(ordoArchive(root, repositoryRoot));
  if (includeArcadia) apps.push(arcadiaArchive(root, repositoryRoot));
  return sign(root, repositoryRoot, apps);
}
