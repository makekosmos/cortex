import { createHash } from "node:crypto";
import { execFileSync } from "node:child_process";
import fs from "node:fs";
import path from "node:path";

type SignedApps = {
  archives: Record<string, string>;
  versions: Record<string, string>;
  catalog: string;
  signatures: unknown;
  trust: { root: string; releases: string };
};

// Test-only fixture keys. They are never accepted by a production build.
const TEST_ONLY_ROOT = {
  privateKey:
    "-----BEGIN PRIVATE KEY-----\nMC4CAQAwBQYDK2VwBCIEIJaqYBUS6pxYArjIJFIVFSBqYEckyyyneug7j00UAjye\n-----END PRIVATE KEY-----\n",
  publicKey: "kO9VLTsXJ61nEokkkuDWvlh7iar3IChCTX1NITSlCLU=",
};
const TEST_ONLY_RELEASE = {
  privateKey:
    "-----BEGIN PRIVATE KEY-----\nMC4CAQAwBQYDK2VwBCIEIOySB4fj+9fjjYqVGN0MUgvLCskThB42RZM33lFKNGId\n-----END PRIVATE KEY-----\n",
  publicKey: "38NFuh0ZiMf4CFadsii2MYZhb3+nIZgqMelepE8Xjho=",
};

const command = (file: string, args: string[], cwd: string) =>
  execFileSync(file, args, { cwd, encoding: "utf8", stdio: "pipe" });

const ps = (value: string) => `'${value.replaceAll("'", "''")}'`;
const zipDirectory = (stage: string, file: string, cwd: string) =>
  command(
    "powershell.exe",
    [
      "-NoProfile",
      "-NonInteractive",
      "-Command",
      `$stage=${ps(stage)}; $file=${ps(file)}; Add-Type -AssemblyName System.IO.Compression; Add-Type -AssemblyName System.IO.Compression.FileSystem; $stream=[IO.File]::Open($file,[IO.FileMode]::Create); try { $zip=[IO.Compression.ZipArchive]::new($stream,[IO.Compression.ZipArchiveMode]::Create,$false); try { Get-ChildItem -LiteralPath $stage -Recurse -File | ForEach-Object { $entry=$zip.CreateEntry($_.FullName.Substring($stage.Length + 1).Replace('\\','/'),[IO.Compression.CompressionLevel]::Optimal); $input=[IO.File]::OpenRead($_.FullName); $output=$entry.Open(); try { $input.CopyTo($output) } finally { $output.Dispose(); $input.Dispose() } } } finally { $zip.Dispose() } } finally { $stream.Dispose() }`,
    ],
    cwd,
  );

const archive = (
  root: string,
  id: string,
  permissions: unknown[],
  closeOnLoad = false,
): { file: string; manifest: Record<string, unknown> } => {
  const stage = path.join(root, id);
  const file = path.join(root, `${id}.kspkg`);
  const typedArkData = permissions.some(
    (permission) =>
      typeof permission === "object" &&
      permission !== null &&
      (permission as { capability?: unknown }).capability === "ark.write",
  )
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
  zipDirectory(stage, file, root);
  return { file, manifest };
};

const packageDirectory = (
  root: string,
  repositoryRoot: string,
  id: string,
  source: string,
  manifest: Record<string, unknown>,
) => {
  const stage = path.join(root, id);
  const file = path.join(root, `${id}.kspkg`);
  fs.cpSync(source, path.join(stage, "dist"), { recursive: true });
  fs.writeFileSync(path.join(stage, "manifest.json"), JSON.stringify(manifest));
  if (typeof manifest.icon === "string") {
    const productIcon = path.join(path.dirname(source), ...manifest.icon.split("/"));
    const icon = fs.existsSync(productIcon)
      ? productIcon
      : path.join(repositoryRoot, "platform", "desktop", "build", "icon.ico");
    const target = path.join(stage, ...manifest.icon.split("/"));
    fs.mkdirSync(path.dirname(target), { recursive: true });
    fs.copyFileSync(icon, target);
  }
  zipDirectory(stage, file, root);
  return { file, manifest };
};

const sign = (
  root: string,
  repositoryRoot: string,
  apps: Array<{ file: string; manifest: Record<string, unknown> }>,
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
      path.join(repositoryRoot, "platform", "desktop", "scripts", "package-sign.mjs"),
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
    archives: Object.fromEntries(apps.map(({ file, manifest }) => [manifest.id as string, file])),
    versions: Object.fromEntries(
      apps.map(({ manifest }) => [manifest.id as string, String(manifest.version)]),
    ),
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
  const manifest = JSON.parse(
    fs.readFileSync(path.join(graphRoot, "manifest.json"), "utf8"),
  ) as Record<string, unknown>;
  return packageDirectory(root, repositoryRoot, String(manifest.id), dist, manifest);
}

function shellArchive(root: string, repositoryRoot: string) {
  const shellRoot = path.join(repositoryRoot, "products", "shell");
  const dist = path.join(shellRoot, "dist");
  if (!fs.existsSync(path.join(dist, "index.html")))
    throw new Error("build products/shell before its Host E2E");
  const manifest = JSON.parse(
    fs.readFileSync(path.join(shellRoot, "manifest.json"), "utf8"),
  ) as Record<string, unknown>;
  return packageDirectory(root, repositoryRoot, String(manifest.id), dist, manifest);
}

export function createSignedApps(
  root: string,
  repositoryRoot: string,
  includeGraph = false,
  includeShell = false,
  closeFixture = false,
): SignedApps {
  const apps: Array<{ file: string; manifest: Record<string, unknown> }> = [
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
  return sign(root, repositoryRoot, apps);
}
