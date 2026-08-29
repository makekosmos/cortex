import { execFileSync } from "node:child_process";
import { createHash } from "node:crypto";
import fs from "node:fs";
import path from "node:path";

const DIGEST = "401781dd9ad396455068546926fc95a16ea772cca42d967114cf60b746afe3e3";
const EXPECTED_MANIFEST = {
  schema_version: 2,
  id: "com.kosmos.focus",
  name: "Ordo",
  version: "0.1.3",
  kind: "app",
  engine_api: ">=1.0.0",
  entrypoint: "dist/index.html",
  icon: "icon.png",
  publisher: "kosmos",
  permissions: [
    {
      capability: "ark.read",
      scopes: [
        "focus.list_blocklists",
        "focus.get_active_state",
        "focus.resolve_blocklist_domains",
        "pomodoro.get_state",
      ],
    },
    {
      capability: "ark.write",
      scopes: [
        "focus.upsert_blocklist",
        "focus.delete_blocklist",
        "focus.set_active_state",
        "pomodoro.start",
        "pomodoro.pause",
        "pomodoro.resume",
        "pomodoro.stop",
      ],
    },
  ],
  targets: [{ runtime: "kosmos-host", os: ["windows"] }],
  data: { access: [], defines: [], mappings: [] },
};

const normalizeEntry = (entry: string) => entry.replaceAll("\\", "/");

const isUnsafeEntry = (entry: string) => {
  const normalized = normalizeEntry(entry);
  return (
    normalized.startsWith("/") ||
    /^[A-Za-z]:\//.test(normalized) ||
    normalized.split("/").includes("..")
  );
};

export function ordoArchive(root: string, repositoryRoot: string) {
  const source = path.join(repositoryRoot, "focus", "release", "ordo-0.1.3.kspkg");
  if (!fs.existsSync(source)) throw new Error(`Ordo release archive not found: ${source}`);
  const file = path.join(root, path.basename(source));
  fs.copyFileSync(source, file);
  if (createHash("sha256").update(fs.readFileSync(file)).digest("hex") !== DIGEST)
    throw new Error("Ordo release archive digest does not match the reviewed artifact");

  const tar = (...args: string[]) =>
    execFileSync("tar", args, { cwd: root, encoding: "utf8", stdio: "pipe" });
  const entries = tar("-tf", file).split(/\r?\n/).filter(Boolean).map(normalizeEntry);
  if (entries.filter((entry) => entry === "manifest.json").length !== 1)
    throw new Error("Ordo archive must contain exactly one manifest.json");
  if (entries.some(isUnsafeEntry)) throw new Error("Ordo archive contains an unsafe path");
  for (const required of ["manifest.json", "icon.png", "dist/index.html"])
    if (!entries.includes(required)) throw new Error(`Ordo archive is missing ${required}`);

  // SAFETY: the archive has exactly one manifest.json and its bytes are JSON-encoded.
  const manifest = JSON.parse(tar("-xOf", file, "manifest.json")) as typeof EXPECTED_MANIFEST;
  if (JSON.stringify(manifest) !== JSON.stringify(EXPECTED_MANIFEST))
    throw new Error("Ordo archive manifest does not match the reviewed v2 contract");
  return { file, manifest };
}
