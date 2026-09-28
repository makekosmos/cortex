import test from "node:test";
import assert from "node:assert/strict";
import { mkdtemp, writeFile, readFile, mkdir } from "node:fs/promises";
import { tmpdir } from "node:os";
import path from "node:path";
import { execFile } from "node:child_process";
import { promisify } from "node:util";
import { generateKeyPairSync, verify as verifySignature } from "node:crypto";
import { fileURLToPath } from "node:url";
const run = promisify(execFile);
const dir = path.dirname(fileURLToPath(import.meta.url));
const input = {
  schema_version: 1,
  sequence: 1,
  issued_at: "2026-01-01T00:00:00Z",
  expires_at: "2027-01-01T00:00:00Z",
  packages: [
    {
      manifest: {
        schema_version: 2,
        id: "demo",
        name: "Demo",
        version: "1.0.0",
        kind: "app",
        engine_api: "^1.0.0",
        entrypoint: "index.js",
        publisher: "kosmos",
        permissions: [],
        targets: [{ runtime: "mundus-host", os: ["windows"] }],
        data: { access: [], defines: [], mappings: [] },
      },
      archive_url: "https://example.test/demo.kspkg",
      sha256: "a".repeat(64),
      size: 3,
    },
  ],
};

test("catalog bytes are deterministic", async () => {
  const t = await mkdtemp(path.join(tmpdir(), "kspkg-"));
  await writeFile(path.join(t, "in.json"), JSON.stringify(input));
  await run(process.execPath, [
    path.join(dir, "package-catalog.mjs"),
    "--input",
    path.join(t, "in.json"),
    "--output",
    path.join(t, "out.json"),
  ]);
  assert.equal((await readFile(path.join(t, "out.json"), "utf8")).endsWith("\n"), true);
});

test("missing signing key fails without key content", async () => {
  const t = await mkdtemp(path.join(tmpdir(), "kspkg-"));
  await writeFile(path.join(t, "in.json"), "{}\n");
  const missing = path.join(t, "missing-secret.pem");
  await assert.rejects(
    run(process.execPath, [
      path.join(dir, "package-sign.mjs"),
      "--input",
      path.join(t, "in.json"),
      "--output",
      path.join(t, "out.json"),
      "--key",
      missing,
      "--key-id",
      "test",
    ]),
    (error) =>
      !String(error.stdout).includes("PRIVATE") &&
      !String(error.stderr).includes("PRIVATE") &&
      !String(error.stderr).includes(missing),
  );
});

test("Ed25519 detached signature verifies", async () => {
  const t = await mkdtemp(path.join(tmpdir(), "kspkg-"));
  const { privateKey, publicKey } = generateKeyPairSync("ed25519");
  await writeFile(path.join(t, "in.json"), JSON.stringify(input));
  await writeFile(path.join(t, "key.pem"), privateKey.export({ type: "pkcs8", format: "pem" }));
  const result = await run(process.execPath, [
    path.join(dir, "package-sign.mjs"),
    "--input",
    path.join(t, "in.json"),
    "--output",
    path.join(t, "sig.json"),
    "--key",
    path.join(t, "key.pem"),
    "--key-id",
    "test",
  ]);
  const privatePem = await readFile(path.join(t, "key.pem"), "utf8");
  assert.equal(result.stdout.includes(privatePem), false);
  assert.equal(result.stderr.includes(privatePem), false);
  const signature = JSON.parse(await readFile(path.join(t, "sig.json"), "utf8")).signatures[0]
    .signature;
  assert.equal(
    verifySignature(
      null,
      await readFile(path.join(t, "in.json")),
      publicKey,
      Buffer.from(signature, "base64"),
    ),
    true,
  );
});

test("dual signer output verifies for both transition keys", async () => {
  const t = await mkdtemp(path.join(tmpdir(), "kspkg-"));
  const first = generateKeyPairSync("ed25519");
  const second = generateKeyPairSync("ed25519");
  await writeFile(path.join(t, "in.json"), JSON.stringify(input));
  await writeFile(
    path.join(t, "one.pem"),
    first.privateKey.export({ type: "pkcs8", format: "pem" }),
  );
  await writeFile(
    path.join(t, "two.pem"),
    second.privateKey.export({ type: "pkcs8", format: "pem" }),
  );
  await run(process.execPath, [
    path.join(dir, "package-sign.mjs"),
    "--input",
    path.join(t, "in.json"),
    "--output",
    path.join(t, "sig.json"),
    "--signer",
    `old=${path.join(t, "one.pem")}`,
    "--signer",
    `new=${path.join(t, "two.pem")}`,
  ]);
  const signatures = JSON.parse(await readFile(path.join(t, "sig.json"), "utf8")).signatures;
  assert.equal(signatures.length, 2);
  const byId = Object.fromEntries(signatures.map((item) => [item.key_id, item.signature]));
  const payload = await readFile(path.join(t, "in.json"));
  assert.equal(
    verifySignature(null, payload, first.publicKey, Buffer.from(byId.old, "base64")),
    true,
  );
  assert.equal(
    verifySignature(null, payload, second.publicKey, Buffer.from(byId.new, "base64")),
    true,
  );
});

test("catalog rejects incomplete manifest", async () => {
  const t = await mkdtemp(path.join(tmpdir(), "kspkg-"));
  await writeFile(
    path.join(t, "in.json"),
    JSON.stringify({
      ...input,
      packages: [{ ...input.packages[0], manifest: {} }],
    }),
  );
  await assert.rejects(
    run(process.execPath, [
      path.join(dir, "package-catalog.mjs"),
      "--input",
      path.join(t, "in.json"),
      "--output",
      path.join(t, "out.json"),
    ]),
  );
});

test("catalog accepts strict Package v1 manifest v2 data contract", async () => {
  const t = await mkdtemp(path.join(tmpdir(), "kspkg-"));
  const v2 = {
    ...input,
    packages: [
      {
        ...input.packages[0],
        manifest: {
          ...input.packages[0].manifest,
          schema_version: 2,
          targets: [{ runtime: "mundus-host", os: ["windows"] }],
          data: { access: [], defines: [], mappings: [] },
        },
      },
    ],
  };
  await writeFile(path.join(t, "in.json"), JSON.stringify(v2));
  await run(process.execPath, [
    path.join(dir, "package-catalog.mjs"),
    "--input",
    path.join(t, "in.json"),
    "--output",
    path.join(t, "out.json"),
  ]);
  assert.equal(
    JSON.parse(await readFile(path.join(t, "out.json"), "utf8")).packages[0].manifest
      .schema_version,
    2,
  );
});

test("catalog rejects legacy Manifest v1", async () => {
  const t = await mkdtemp(path.join(tmpdir(), "kspkg-v1-"));
  await writeFile(
    path.join(t, "in.json"),
    JSON.stringify({
      ...input,
      packages: [
        { ...input.packages[0], manifest: { ...input.packages[0].manifest, schema_version: 1 } },
      ],
    }),
  );
  await assert.rejects(
    run(process.execPath, [
      path.join(dir, "package-catalog.mjs"),
      "--input",
      path.join(t, "in.json"),
      "--output",
      path.join(t, "out.json"),
    ]),
  );
});

test("package app stages and signs a v2 archive", { timeout: 15_000 }, async () => {
  const t = await mkdtemp(path.join(tmpdir(), "kspkg-app-"));
  const source = path.join(t, "dist");
  await mkdir(source, { recursive: true });
  await writeFile(path.join(source, "index.html"), "<title>Mundus</title>");
  const manifest = {
    ...input.packages[0].manifest,
    schema_version: 2,
    id: "com.kosmos.demo",
    targets: [{ runtime: "mundus-host", os: ["windows"] }],
    data: { access: [], defines: [], mappings: [] },
    entrypoint: "dist/index.html",
  };
  await writeFile(path.join(t, "manifest.json"), JSON.stringify(manifest));
  const { privateKey, publicKey } = generateKeyPairSync("ed25519");
  await writeFile(path.join(t, "release.pem"), privateKey.export({ type: "pkcs8", format: "pem" }));
  await run(process.execPath, [
    path.join(dir, "package-app.mjs"),
    "--source",
    source,
    "--manifest",
    path.join(t, "manifest.json"),
    "--archive",
    path.join(t, "demo.kspkg"),
    "--catalog",
    path.join(t, "catalog.json"),
    "--signatures",
    path.join(t, "signatures.json"),
    "--archive-url",
    "https://example.test/{id}.kspkg",
    "--sequence",
    "1",
    "--issued-at",
    "2026-01-01T00:00:00Z",
    "--expires-at",
    "2027-01-01T00:00:00Z",
    "--signer",
    `release=${path.join(t, "release.pem")}`,
  ]);
  const catalogBytes = await readFile(path.join(t, "catalog.json"));
  const signatures = JSON.parse(await readFile(path.join(t, "signatures.json"), "utf8"));
  assert.equal(signatures.signatures.length, 1);
  assert.equal(
    verifySignature(
      null,
      catalogBytes,
      publicKey,
      Buffer.from(signatures.signatures[0].signature, "base64"),
    ),
    true,
  );
  assert.equal(JSON.parse(catalogBytes).packages[0].manifest.schema_version, 2);
});
