import { createHash } from "node:crypto";
import { readFileSync } from "node:fs";
import path from "node:path";

export interface ReleaseBomIdentity {
  id: string;
  digest: string;
}

interface ReleaseBomDocument {
  id?: unknown;
}

export function readEmbeddedReleaseBomIdentity(): ReleaseBomIdentity | undefined {
  try {
    const raw = readFileSync(path.join(process.resourcesPath, "release-bom.json"));
    const value: unknown = JSON.parse(raw.toString("utf8"));
    if (!value || Object.prototype.toString.call(value) !== "[object Object]") return undefined;
    // SAFETY: the object representation is checked above; `id` remains unknown until checked below.
    const id = (value as ReleaseBomDocument).id;
    if (Object.prototype.toString.call(id) !== "[object String]" || !id) return undefined;
    return {
      id: String(id),
      digest: createHash("sha256").update(raw).digest("hex"),
    };
  } catch {
    return undefined;
  }
}
