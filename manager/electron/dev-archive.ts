import fs from "node:fs";
import { inflateRawSync } from "node:zlib";
import { isString, type Input } from "./manager-contract";

// Minimal reader for the manifest entry inside a `.kspkg` archive (plain zip).
// Used only on the development-package install path to derive package_id and
// version for `packages.install_development`; the engine re-parses and
// validates the same manifest before anything is installed.

const EOCD_SIGNATURE = 0x06054b50;
const CENTRAL_SIGNATURE = 0x02014b50;
const LOCAL_SIGNATURE = 0x04034b50;
const MAX_ARCHIVE_SIZE = 64 * 1024 * 1024;
const MAX_ENTRY_SIZE = 1024 * 1024;
const MAX_ENTRIES = 4096;

export function readPackageArchiveManifest(file: string): { id: string; version: string } | null {
  try {
    const stat = fs.statSync(file);
    if (!stat.isFile() || stat.size > MAX_ARCHIVE_SIZE) return null;
    const bytes = fs.readFileSync(file);
    const eocd = findEndOfCentralDirectory(bytes);
    if (eocd < 0) return null;
    const entries = bytes.readUInt16LE(eocd + 10);
    let offset = bytes.readUInt32LE(eocd + 16);
    if (entries > MAX_ENTRIES) return null;
    for (let index = 0; index < entries; index += 1) {
      if (offset + 46 > bytes.length || bytes.readUInt32LE(offset) !== CENTRAL_SIGNATURE)
        return null;
      const method = bytes.readUInt16LE(offset + 10);
      const compressedSize = bytes.readUInt32LE(offset + 20);
      const uncompressedSize = bytes.readUInt32LE(offset + 24);
      const nameLength = bytes.readUInt16LE(offset + 28);
      const extraLength = bytes.readUInt16LE(offset + 30);
      const commentLength = bytes.readUInt16LE(offset + 32);
      const localHeader = bytes.readUInt32LE(offset + 42);
      const name = bytes.subarray(offset + 46, offset + 46 + nameLength).toString("utf8");
      if (name === "manifest.json") {
        if (uncompressedSize > MAX_ENTRY_SIZE || localHeader + 30 > bytes.length) return null;
        if (bytes.readUInt32LE(localHeader) !== LOCAL_SIGNATURE) return null;
        const localName = bytes.readUInt16LE(localHeader + 26);
        const localExtra = bytes.readUInt16LE(localHeader + 28);
        const dataStart = localHeader + 30 + localName + localExtra;
        const data = bytes.subarray(dataStart, dataStart + compressedSize);
        const raw = method === 8 ? inflateRawSync(data) : method === 0 ? data : null;
        if (!raw) return null;
        // SAFETY: the parsed manifest is untrusted; the field checks below
        // enforce the package-id/version contract before use.
        const manifest = JSON.parse(raw.toString("utf8")) as { id?: Input; version?: Input };
        const id = isString(manifest.id) ? manifest.id : "";
        const version = isString(manifest.version) ? manifest.version : "";
        return /^[a-z0-9][a-z0-9._-]{0,127}$/.test(id) && version.length > 0 && version.length <= 64
          ? { id, version }
          : null;
      }
      offset += 46 + nameLength + extraLength + commentLength;
    }
    return null;
  } catch {
    return null;
  }
}

function findEndOfCentralDirectory(bytes: Buffer): number {
  const minimum = 22;
  for (
    let offset = bytes.length - minimum;
    offset >= Math.max(0, bytes.length - 65557);
    offset -= 1
  )
    if (bytes.readUInt32LE(offset) === EOCD_SIGNATURE) return offset;
  return -1;
}
