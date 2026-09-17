import { execFileSync } from "node:child_process";
import fs from "node:fs";
import path from "node:path";
import { fileURLToPath } from "node:url";
import { TEST_ONLY_STORE } from "../../host/e2e/fixtures/signing-keys";

const repositoryRoot = path.resolve(path.dirname(fileURLToPath(import.meta.url)), "..", "..");

// Wire shape of the runtime Store Catalog document (`store_catalog.rs` uses
// serde deny_unknown_fields): every non-Option field is required, `connects_to`
// stays null outside integration listings, and `via` names the listing itself
// for non-integration rows.
export type StoreFixtureListing = {
  id: string;
  kind: "kosmos-package" | "integration" | "external-app";
  name: string;
  publisher: string;
  publisher_tier: "kosmos" | "verified" | "community";
  description: string;
  categories: string[];
  availability: { platforms: string[] };
  data_compatibility: Array<{
    type: string;
    versions: string;
    roles: string[];
    via: string;
    fidelity: "native" | "lossless" | "lossy" | "metadata-only";
  }>;
  distribution:
    | { package_id: string; version: string }
    | { official_url: string }
    | { package_id: string; version: string; connects_to: string };
  connects_to: string | null;
  icon_url: string | null;
  screenshots: string[];
};

export const storePackageListing = (
  id: string,
  name: string,
  packageId: string,
  version: string,
): StoreFixtureListing => ({
  id,
  kind: "kosmos-package",
  name,
  publisher: "Kosmos",
  publisher_tier: "kosmos",
  description: "Signed store fixture package listing.",
  categories: ["productivity"],
  availability: { platforms: ["linux", "windows"] },
  data_compatibility: [
    {
      type: "com.kosmos.note",
      versions: ">=1.0.0",
      roles: ["read", "edit"],
      via: id,
      fidelity: "native",
    },
  ],
  distribution: { package_id: packageId, version },
  connects_to: null,
  icon_url: null,
  screenshots: [],
});

export const storeExternalListing = (
  id: string,
  name: string,
  officialUrl: string,
): StoreFixtureListing => ({
  id,
  kind: "external-app",
  name,
  publisher: "",
  publisher_tier: "community",
  description: "Signed store fixture external listing.",
  categories: [],
  availability: { platforms: ["windows"] },
  data_compatibility: [],
  distribution: { official_url: officialUrl },
  connects_to: null,
  icon_url: null,
  screenshots: [],
});

// Signs the document with the test-only store key and persists the envelope to
// `<dataDir>/store/catalog.json` so the Engine loads it at startup
// (`StoreCatalogService::open` → `load_persisted`).
export const seedStoreCatalog = (
  root: string,
  dataDir: string,
  sequence: number,
  issuedAt: string,
  expiresAt: string,
  listings: StoreFixtureListing[],
): string => {
  const document = JSON.stringify({
    schema_version: 1,
    sequence,
    issued_at: issuedAt,
    expires_at: expiresAt,
    listings,
  });
  const stage = path.join(root, "store-catalog");
  fs.mkdirSync(stage, { recursive: true });
  const documentFile = path.join(stage, "document.json");
  const keyFile = path.join(stage, "store.pem");
  const signaturesFile = path.join(stage, "document.signatures.json");
  fs.writeFileSync(documentFile, document);
  fs.writeFileSync(keyFile, TEST_ONLY_STORE.privateKey);
  execFileSync(
    process.execPath,
    [
      path.join(repositoryRoot, "desktop", "scripts", "package-sign.mjs"),
      "--input",
      documentFile,
      "--output",
      signaturesFile,
      "--key-id",
      TEST_ONLY_STORE.keyId,
      "--key",
      keyFile,
    ],
    { cwd: repositoryRoot, stdio: "pipe" },
  );
  // SAFETY: package-sign.mjs writes the SignatureSet shape consumed by the
  // StoreCatalogEnvelope contract.
  const signatures = JSON.parse(fs.readFileSync(signaturesFile, "utf8")) as unknown;
  const envelope = JSON.stringify({
    bytes: Buffer.from(document, "utf8").toString("base64"),
    signatures,
  });
  const storeDir = path.join(dataDir, "store");
  fs.mkdirSync(storeDir, { recursive: true });
  const target = path.join(storeDir, "catalog.json");
  fs.writeFileSync(target, envelope);
  return target;
};
