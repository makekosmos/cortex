import type { Game } from "../../src/types";

export function isSupportedDropPath(path: string) {
  const lower = path.toLowerCase();
  return lower.endsWith(".exe") || lower.endsWith(".lnk");
}

export function fileNameFromPath(filePath: string) {
  const normalized = filePath.replace(/\\/g, "/");
  const name = normalized.split("/").pop();
  return name || filePath;
}

export function cleanNameFromFile(fileName: string) {
  const base = fileName.replace(/\.(exe|lnk)$/i, "");
  return base.replace(/[-_]/g, " ").replace(/\s+/g, " ").trim() || base;
}

export function normalizeNameForMerge(value: string) {
  return value
    .normalize("NFKD")
    .toLowerCase()
    .replace(/\.(exe|lnk)$/i, "")
    .replace(/\[[^\]]*\]|\([^)]*\)|\{[^}]*\}/g, " ")
    .replace(/[-_./]/g, " ")
    .replace(/[^\p{L}\p{N}\s]/gu, " ")
    .replace(/\s+/g, " ")
    .trim();
}

export function matchMergeNames(candidate: string, incoming: string) {
  if (!candidate || !incoming) return false;
  if (candidate === incoming) return true;

  if (
    candidate.length > 4 &&
    incoming.length > 4 &&
    (candidate.includes(incoming) || incoming.includes(candidate))
  ) {
    return true;
  }

  const candidateTokens = candidate.split(" ").filter(Boolean);
  const incomingTokens = incoming.split(" ").filter(Boolean);
  if (candidateTokens.length <= 1 || incomingTokens.length <= 1) return false;

  const tokenSet = new Set(incomingTokens);
  const overlap = candidateTokens.filter((token) => tokenSet.has(token)).length;
  if (overlap < 2) return false;

  return overlap / Math.max(candidateTokens.length, incomingTokens.length) >= 0.67;
}

export function findMergeCandidate(
  games: Pick<Game, "name" | "exe_name" | "exe_path">[],
  name: string,
): Game | undefined {
  const normalized = normalizeNameForMerge(name);
  if (!normalized) return undefined;

  for (const game of games) {
    const candidateNames = [game.name, game.exe_name, fileNameFromPath(game.exe_path)];
    const exact = candidateNames.find(
      (candidateName) => normalizeNameForMerge(candidateName) === normalized,
    );
    if (exact) return game as Game;
  }

  return games.find((game) =>
    [game.name, game.exe_name, fileNameFromPath(game.exe_path)].some(
      (candidateName) => {
        const normalizedCandidate = normalizeNameForMerge(candidateName);
        return matchMergeNames(normalizedCandidate, normalized);
      },
    ),
  ) as Game | undefined;
}
