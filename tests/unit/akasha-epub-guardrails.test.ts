import { describe, expect, test } from "bun:test";
import { readEpubBytes } from "../../incubator/akasha/src/lib/epub";

function makeZipWithSingleEntry(name: string, uncompressedSize: number): Uint8Array {
  const encoder = new TextEncoder();
  const nameBytes = encoder.encode(name);
  const localSize = 30 + nameBytes.length;
  const centralOffset = localSize;
  const centralSize = 46 + nameBytes.length;
  const eocdOffset = centralOffset + centralSize;
  const bytes = new Uint8Array(eocdOffset + 22);
  const view = new DataView(bytes.buffer);

  view.setUint32(0, 0x04034b50, true);
  view.setUint16(26, nameBytes.length, true);
  bytes.set(nameBytes, 30);

  view.setUint32(centralOffset, 0x02014b50, true);
  view.setUint32(centralOffset + 24, uncompressedSize, true);
  view.setUint16(centralOffset + 28, nameBytes.length, true);
  view.setUint32(centralOffset + 42, 0, true);
  bytes.set(nameBytes, centralOffset + 46);

  view.setUint32(eocdOffset, 0x06054b50, true);
  view.setUint16(eocdOffset + 10, 1, true);
  view.setUint32(eocdOffset + 12, centralSize, true);
  view.setUint32(eocdOffset + 16, centralOffset, true);

  return bytes;
}

function makeZipWithEntryCount(entryCount: number): Uint8Array {
  const bytes = new Uint8Array(22);
  const view = new DataView(bytes.buffer);
  view.setUint32(0, 0x06054b50, true);
  view.setUint16(10, entryCount, true);
  view.setUint32(16, 0, true);
  return bytes;
}

describe("Akasha EPUB guardrails", () => {
  test("rejects oversized source files before ZIP parsing", async () => {
    await expect(readEpubBytes(new Uint8Array(80 * 1024 * 1024 + 1), "huge.epub")).rejects.toThrow(
      "до 80 МБ",
    );
  });

  test("rejects archives with too many entries", async () => {
    await expect(readEpubBytes(makeZipWithEntryCount(4_001), "many.epub")).rejects.toThrow(
      "слишком много файлов",
    );
  });

  test("rejects entries with oversized uncompressed payloads", async () => {
    await expect(
      readEpubBytes(makeZipWithSingleEntry("chapter.xhtml", 20 * 1024 * 1024 + 1), "bomb.epub"),
    ).rejects.toThrow("превышает лимит размера");
  });
});
