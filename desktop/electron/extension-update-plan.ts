import type { InstalledExtensionInfo } from "./extension-installer";

export interface ExtensionUpdateCandidate {
  id: string;
  name: string;
  currentVersion: string;
  nextVersion: string;
  downloadUrl: string;
  sha256: string | null;
}

interface SemVer {
  major: number;
  minor: number;
  patch: number;
}

interface UpdateCatalog {
  extensions: Array<{
    id: string;
    appId: string | null;
    version: string;
    downloadUrl: string;
    sha256: string | null;
  }>;
}

function parseSemver(v: string | null | undefined): SemVer | null {
  if (!v) return null;
  const m = /^(\d+)\.(\d+)\.(\d+)$/.exec(v.trim());
  if (!m) return null;
  return {
    major: Number(m[1]),
    minor: Number(m[2]),
    patch: Number(m[3]),
  };
}

function compareSemver(a: SemVer, b: SemVer): number {
  if (a.major !== b.major) return a.major - b.major;
  if (a.minor !== b.minor) return a.minor - b.minor;
  return a.patch - b.patch;
}

export function findExtensionUpdates(
  installed: InstalledExtensionInfo[],
  catalog: UpdateCatalog,
): ExtensionUpdateCandidate[] {
  const byAppId = new Map<string, UpdateCatalog["extensions"][number]>();
  for (const entry of catalog.extensions) {
    if (!entry.appId) continue;
    byAppId.set(entry.appId, entry);
  }
  const out: ExtensionUpdateCandidate[] = [];

  for (const ext of installed) {
    if (ext.source !== "installed") continue;
    if (!ext.appId) continue;
    const entry = byAppId.get(ext.appId);
    if (!entry) continue;

    const current = parseSemver(ext.version);
    const next = parseSemver(entry.version);
    if (!current || !next) continue;
    if (compareSemver(next, current) <= 0) continue;

    out.push({
      id: ext.id,
      name: ext.name,
      currentVersion: ext.version!,
      nextVersion: entry.version,
      downloadUrl: entry.downloadUrl,
      sha256: entry.sha256,
    });
  }

  return out;
}
