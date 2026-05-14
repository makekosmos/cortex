// @deprecated 2026-05-15: концепция spaces убрана, single DB per user.
// Module оставлен временно для legacy/dashboard-extension и mobile/delphi
// (могут читать старый selected-space.json для миграции). Активный код
// shell/kepler-backend больше эти helper'ы не использует — все пишут в
// %APPDATA%/Kosmos/ark.db без space resolution.
// Удалить когда последний consumer переедет.

import { createHash } from "node:crypto";
import fs from "node:fs";
import path from "node:path";

const CROCKFORD = "0123456789ABCDEFGHJKMNPQRSTVWXYZ";

export interface SharedSelectedSpace {
  version: 1;
  spaceCode: string;
  spaceId: string;
  vaultPath: string | null;
  source: string;
  updatedAt: string;
}

function normalizeSpaceCode(code: string): string {
  return code.replace(/[-\s]/g, "").toUpperCase();
}

function normalizeVaultPath(vaultPath: string): string {
  const resolved = path.resolve(vaultPath).replaceAll("/", "\\");
  return process.platform === "win32" ? resolved.toLowerCase() : resolved;
}

export function deriveSpaceIdFromCode(code: string): string {
  const normalized = normalizeSpaceCode(code);
  return createHash("sha256").update(normalized, "utf8").digest("hex").slice(0, 16);
}

export function derivePersonalSpaceCodeFromVaultPath(vaultPath: string): string {
  const digest = createHash("sha256").update(normalizeVaultPath(vaultPath), "utf8").digest();
  let buffer = 0;
  let bits = 0;
  let output = "";

  for (const byte of digest) {
    buffer = (buffer << 8) | byte;
    bits += 8;

    while (bits >= 5 && output.length < 12) {
      bits -= 5;
      output += CROCKFORD[(buffer >> bits) & 31];
    }

    if (output.length >= 12) {
      break;
    }
  }

  while (output.length < 12) {
    output += CROCKFORD[0];
  }

  return output;
}

export function getKosmosDataDir(appDataPath: string): string {
  return path.join(appDataPath, "Kosmos");
}

export function getSharedSelectedSpacePath(appDataPath: string): string {
  return path.join(getKosmosDataDir(appDataPath), "selected-space.json");
}

export function readSharedSelectedSpace(appDataPath: string): SharedSelectedSpace | null {
  const filePath = getSharedSelectedSpacePath(appDataPath);
  if (!fs.existsSync(filePath)) {
    return null;
  }

  try {
    // PowerShell `Set-Content -Encoding utf8` на Windows PS 5.1 пишет UTF-8
    // с BOM (EF BB BF). JSON.parse кидает SyntaxError на BOM — strip его
    // явно, иначе следующий agent / installer силенцно сломает space resolution.
    const raw = fs.readFileSync(filePath, "utf8").replace(/^﻿/, "");
    const parsed = JSON.parse(raw) as Partial<SharedSelectedSpace>;
    if (
      parsed?.version !== 1 ||
      typeof parsed.spaceCode !== "string" ||
      typeof parsed.spaceId !== "string" ||
      typeof parsed.source !== "string" ||
      typeof parsed.updatedAt !== "string"
    ) {
      return null;
    }

    return {
      version: 1,
      spaceCode: normalizeSpaceCode(parsed.spaceCode),
      spaceId: parsed.spaceId,
      vaultPath: typeof parsed.vaultPath === "string" ? parsed.vaultPath : null,
      source: parsed.source,
      updatedAt: parsed.updatedAt,
    };
  } catch {
    return null;
  }
}

export function writeSharedSelectedSpace(
  appDataPath: string,
  selection: SharedSelectedSpace | null,
): void {
  const filePath = getSharedSelectedSpacePath(appDataPath);
  fs.mkdirSync(path.dirname(filePath), { recursive: true });

  if (!selection) {
    if (fs.existsSync(filePath)) {
      fs.unlinkSync(filePath);
    }
    return;
  }

  fs.writeFileSync(filePath, JSON.stringify(selection, null, 2), "utf8");
}

export function buildPersonalSelectedSpace(
  vaultPath: string,
  source: string,
  now: Date = new Date(),
): SharedSelectedSpace {
  const normalizedVaultPath = path.resolve(vaultPath);
  const spaceCode = derivePersonalSpaceCodeFromVaultPath(normalizedVaultPath);

  return {
    version: 1,
    spaceCode,
    spaceId: deriveSpaceIdFromCode(spaceCode),
    vaultPath: normalizedVaultPath,
    source,
    updatedAt: now.toISOString(),
  };
}

export function buildSharedSelectedSpaceFromCode(
  spaceCode: string,
  source: string,
  options: {
    vaultPath?: string | null;
    now?: Date;
  } = {},
): SharedSelectedSpace {
  const normalizedCode = normalizeSpaceCode(spaceCode);

  return {
    version: 1,
    spaceCode: normalizedCode,
    spaceId: deriveSpaceIdFromCode(normalizedCode),
    vaultPath: options.vaultPath ? path.resolve(options.vaultPath) : null,
    source,
    updatedAt: (options.now ?? new Date()).toISOString(),
  };
}

export function getArkDbPathForSelectedSpace(
  appDataPath: string,
  selection: SharedSelectedSpace | null,
): string {
  const dataDir = getKosmosDataDir(appDataPath);
  if (!selection?.spaceId) {
    return path.join(dataDir, "ark.db");
  }

  return path.join(dataDir, "spaces", selection.spaceId, "ark.db");
}
