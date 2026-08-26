import fs from "node:fs";
import path from "node:path";

export type DevPackage = {
  id: string;
  name: string;
  version: string;
  publisher: string;
  iconPath: string;
  archivePath: string;
  url: string;
};

type DevPackageConfig = {
  packages: Array<{ path: string; url: string }>;
};

type PackageManifest = {
  id: string;
  name: string;
  version: string;
  publisher?: string;
  icon: string;
};

const safeId = /^[a-z0-9][a-z0-9._-]{0,127}$/;
function localUrl(value: string): string | null {
  try {
    const url = new URL(value);
    return url.protocol === "http:" && ["127.0.0.1", "localhost"].includes(url.hostname) && url.port
      ? url.toString()
      : null;
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
    return config.packages.flatMap(({ path: relativePath, url: sourceUrl }) => {
      const url = localUrl(sourceUrl);
      if (!relativePath || !url) return [];
      const root = path.resolve(path.dirname(configPath), relativePath);
      // SAFETY: each manifest was validated before its dev package was built.
      const manifest = JSON.parse(fs.readFileSync(path.join(root, "manifest.json"), "utf8")) as PackageManifest;
      const { id, name, version, icon } = manifest;
      if (!safeId.test(id) || !name || !version || !icon) return [];
      const release = path.join(root, "release");
      const archive = fs.readdirSync(release)
        .filter((file) => file.endsWith(`-${version}.kspkg`))
        .map((file) => path.join(release, file))
        .find((file) => fs.statSync(file).isFile());
      if (!archive) return [];
      return [{
        id,
        name,
        version,
        publisher: manifest.publisher?.trim() || "Kosmos",
        iconPath: path.join(root, icon),
        archivePath: archive,
        url,
      }];
    });
  } catch {
    return [];
  }
}

export function developmentPackage(id: string): DevPackage | undefined {
  return developmentPackages().find((entry) => entry.id === id);
}
