import * as fs from "node:fs/promises";
import path from "node:path";
import zlib from "node:zlib";

export interface ZipEntry {
  name: string;
  isDir: boolean;
  data: Buffer;
}

const EOCD_SIG = 0x06054b50;
const CDFH_SIG = 0x02014b50;
const LFH_SIG = 0x04034b50;

function findEOCD(buf: Buffer): { cdOffset: number; cdEntries: number } | null {
  const minOffset = Math.max(0, buf.length - 65557);
  for (let i = buf.length - 22; i >= minOffset; i--) {
    if (buf.readUInt32LE(i) === EOCD_SIG) {
      const cdEntries = buf.readUInt16LE(i + 10);
      const cdOffset = buf.readUInt32LE(i + 16);
      const cdSize = buf.readUInt32LE(i + 12);
      if (cdEntries === 0xffff || cdOffset === 0xffffffff || cdSize === 0xffffffff) {
        throw new Error("zip: ZIP64 not supported");
      }
      return { cdOffset, cdEntries };
    }
  }
  return null;
}

export async function readZipEntries(zipPath: string): Promise<ZipEntry[]> {
  const buf = await fs.readFile(zipPath);
  const eocd = findEOCD(buf);
  if (!eocd) throw new Error(`not a valid zip (no EOCD): ${zipPath}`);
  const entries: ZipEntry[] = [];
  let offset = eocd.cdOffset;
  for (let i = 0; i < eocd.cdEntries; i++) {
    if (buf.readUInt32LE(offset) !== CDFH_SIG) {
      throw new Error(`zip: bad central dir header at ${offset}`);
    }
    const compMethod = buf.readUInt16LE(offset + 10);
    const compSize = buf.readUInt32LE(offset + 20);
    const uncompSize = buf.readUInt32LE(offset + 24);
    const nameLen = buf.readUInt16LE(offset + 28);
    const extraLen = buf.readUInt16LE(offset + 30);
    const commentLen = buf.readUInt16LE(offset + 32);
    const lfhOffset = buf.readUInt32LE(offset + 42);
    const name = buf.subarray(offset + 46, offset + 46 + nameLen).toString("utf8");
    if (buf.readUInt32LE(lfhOffset) !== LFH_SIG) {
      throw new Error(`zip: bad local header at ${lfhOffset} for ${name}`);
    }
    const lfhNameLen = buf.readUInt16LE(lfhOffset + 26);
    const lfhExtraLen = buf.readUInt16LE(lfhOffset + 28);
    const dataStart = lfhOffset + 30 + lfhNameLen + lfhExtraLen;
    const rawData = buf.subarray(dataStart, dataStart + compSize);
    let data: Buffer;
    if (name.endsWith("/") || uncompSize === 0) {
      data = Buffer.alloc(0);
    } else if (compMethod === 0) {
      data = Buffer.from(rawData);
    } else if (compMethod === 8) {
      data = zlib.inflateRawSync(rawData);
    } else {
      throw new Error(`zip: unsupported compression ${compMethod} for ${name}`);
    }
    entries.push({ name, isDir: name.endsWith("/"), data });
    offset += 46 + nameLen + extraLen + commentLen;
  }
  return entries;
}

function safeEntryName(name: string): string {
  if (!name) throw new Error("zip: empty entry name");
  const norm = name.replace(/\\/g, "/");
  if (norm.startsWith("/")) throw new Error(`zip: absolute path: ${name}`);
  if (/^[a-zA-Z]:/.test(norm)) throw new Error(`zip: drive letter: ${name}`);
  for (const p of norm.split("/")) {
    if (p === "..") throw new Error(`zip: parent traversal: ${name}`);
  }
  return norm;
}

export async function extractZipTo(zipPath: string, targetDir: string): Promise<void> {
  const entries = await readZipEntries(zipPath);
  await fs.mkdir(targetDir, { recursive: true });
  for (const e of entries) {
    const safe = safeEntryName(e.name);
    const out = path.join(targetDir, safe);
    if (e.isDir) {
      await fs.mkdir(out, { recursive: true });
      continue;
    }
    await fs.mkdir(path.dirname(out), { recursive: true });
    await fs.writeFile(out, e.data);
  }
}
