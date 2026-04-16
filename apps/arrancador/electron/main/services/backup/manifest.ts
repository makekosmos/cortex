import { readFile, writeFile } from "node:fs/promises";
import path from "node:path";
import type { GameManifest, GameManifestEntry } from "./types";

const NORMALIZE_RE = /[^a-z0-9]+/g;

export function normalizeName(name: string): string {
  const stopWords = new Set([
    "the",
    "a",
    "an",
    "edition",
    "definitive",
    "remastered",
    "goty",
    "game",
    "of",
    "year",
    "ultimate",
    "complete",
    "collection",
    "bundle",
    "deluxe",
    "enhanced",
    "hd",
  ]);

  const cleaned = name.toLowerCase().replace(NORMALIZE_RE, " ");
  const tokens = cleaned
    .split(/\s+/)
    .map((token) => token.trim())
    .filter((token) => token.length > 0 && !stopWords.has(token));

  return tokens.join(" ");
}

export function similarityScore(a: string, b: string): number {
  if (!a || !b) {
    return 0;
  }
  if (a === b) {
    return 1;
  }
  if (a.includes(b) || b.includes(a)) {
    return 0.9;
  }

  const setA = new Set(a.split(/\s+/).filter(Boolean));
  const setB = new Set(b.split(/\s+/).filter(Boolean));
  if (setA.size === 0 || setB.size === 0) {
    return 0;
  }

  let intersection = 0;
  for (const value of setA) {
    if (setB.has(value)) {
      intersection += 1;
    }
  }

  const union = new Set([...setA, ...setB]).size;
  return union === 0 ? 0 : intersection / union;
}

export function findGameEntry(
  manifest: GameManifest | null | undefined,
  name: string,
): [string, GameManifestEntry] | null {
  if (!manifest) {
    return null;
  }

  const exact = manifest.games[name];
  if (exact) {
    return [name, exact];
  }

  const normalizedTarget = normalizeName(name);
  const normalizedExact = new Map<string, string>();
  const normalizedKeys: Array<[string, string]> = [];

  for (const key of Object.keys(manifest.games)) {
    const normalized = normalizeName(key);
    if (!normalizedExact.has(normalized)) {
      normalizedExact.set(normalized, key);
    }
    normalizedKeys.push([key, normalized]);
  }

  const directKey = normalizedExact.get(normalizedTarget);
  if (directKey) {
    return [directKey, manifest.games[directKey]];
  }

  let bestKey: string | null = null;
  let bestScore = 0;
  for (const [key, normalized] of normalizedKeys) {
    const score = similarityScore(normalizedTarget, normalized);
    if (score > bestScore) {
      bestKey = key;
      bestScore = score;
    }
  }

  if (bestKey && bestScore >= 0.6) {
    return [bestKey, manifest.games[bestKey]];
  }

  return null;
}

export function suggestGames(
  manifest: GameManifest | null | undefined,
  name: string,
  limit: number,
): string[] {
  if (!manifest) {
    return [];
  }

  const normalizedTarget = normalizeName(name);
  const scored = Object.keys(manifest.games)
    .map((key) => [key, similarityScore(normalizedTarget, normalizeName(key))] as const)
    .filter(([, score]) => score >= 0.4)
    .sort((a, b) => b[1] - a[1]);

  return scored.slice(0, Math.max(0, limit)).map(([key]) => key);
}

export async function loadGameManifestCache(cachePath: string): Promise<GameManifest | null> {
  try {
    const text = await readFile(cachePath, "utf8");
    const parsed = JSON.parse(text) as GameManifest;
    if (!parsed || typeof parsed !== "object" || !parsed.games) {
      return null;
    }
    return parsed;
  } catch {
    return null;
  }
}

export async function writeGameManifestCache(
  cachePath: string,
  manifest: GameManifest,
): Promise<void> {
  await writeFile(cachePath, JSON.stringify(manifest, null, 2), "utf8");
}

export function manifestCachePath(baseDir: string): string {
  return path.join(baseDir, "sqoba_manifest.json");
}
