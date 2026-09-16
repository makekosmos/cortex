import fs from "node:fs";
import path from "node:path";

export type DevPackage = {
  id: string;
  name: string;
  version: string;
  publisher: string;
  iconPath: string;
  archivePath: string;
  sourcePath: string;
  url?: string;
};

type DevPackageConfig = {
  packages: Array<{ path: string; url?: string }>;
};

type PackageManifest = {
  id: string;
  name: string;
  version: string;
  publisher?: string;
  icon?: string;
};

const safeId = /^[a-z0-9][a-z0-9._-]{0,127}$/;
export function localUrl(value: string): string | null {
  try {
    const url = new URL(value);
    return url.protocol === "http:" && ["127.0.0.1", "localhost"].includes(url.hostname) && url.port
      ? url.toString()
      : null;
  } catch {
    return null;
  }
}

// Resolves a package source directory into an installable dev package: the
// manifest identifies the package and its version, the release directory
// supplies the built archive. `url` is only needed for app packages that are
// served by a local dev server.
export function resolveDevelopmentPackageDir(root: string, sourceUrl?: string): DevPackage | null {
  try {
    const url = sourceUrl === undefined ? undefined : (localUrl(sourceUrl) ?? undefined);
    if (sourceUrl !== undefined && url === undefined) return null;
    const manifestPath = path.join(root, "package.manifest.json");
    const legacyManifestPath = path.join(root, "manifest.json");
    // SAFETY: the canonical package manifest is preferred when present; legacy repos retain the fallback.
    const manifestFile = fs.existsSync(manifestPath) ? manifestPath : legacyManifestPath;
    // SAFETY: the manifest is untrusted JSON; the field checks below enforce the contract.
    const manifest = JSON.parse(fs.readFileSync(manifestFile, "utf8")) as PackageManifest;
    const { id, name, version, icon } = manifest;
    if (!safeId.test(id) || !name || !version) return null;
    const release = path.join(root, "release");
    const archive = fs
      .readdirSync(release)
      .filter((file) => file.endsWith(`-${version}.kspkg`))
      .map((file) => path.join(release, file))
      .find((file) => fs.statSync(file).isFile());
    if (!archive) return null;
    return {
      id,
      name,
      version,
      publisher: manifest.publisher?.trim() || "Kosmos",
      iconPath: icon ? path.join(root, icon) : "",
      archivePath: archive,
      sourcePath: path.resolve(root),
      url,
    };
  } catch {
    return null;
  }
}

export function developmentPackages(): DevPackage[] {
  if (process.env.KOSMOS_DEV_PACKAGES !== "1") return [];
  try {
    const configPath = path.resolve(import.meta.dirname, "../../dev-packages.json");
    // SAFETY: dev-packages.json is a repository-owned development-only file.
    const config = JSON.parse(fs.readFileSync(configPath, "utf8")) as DevPackageConfig;
    return config.packages.flatMap(({ path: relativePath, url }) => {
      if (!relativePath) return [];
      const root = path.resolve(path.dirname(configPath), relativePath);
      const resolved = resolveDevelopmentPackageDir(root, url);
      return resolved ? [resolved] : [];
    });
  } catch {
    return [];
  }
}

export function developmentPackage(id: string): DevPackage | undefined {
  return developmentPackages().find((entry) => entry.id === id);
}
