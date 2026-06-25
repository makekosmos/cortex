const textDecoder = new TextDecoder("utf-8");
const MAX_EPUB_BYTES = 80 * 1024 * 1024;
const MAX_ZIP_ENTRIES = 4_000;
const MAX_TOTAL_UNCOMPRESSED_BYTES = 200 * 1024 * 1024;
const MAX_ENTRY_UNCOMPRESSED_BYTES = 20 * 1024 * 1024;

export interface ZipEntry {
  name: string;
  method: number;
  compressedSize: number;
  uncompressedSize: number;
  localHeaderOffset: number;
}

export function assertEpubSize(bytes: Uint8Array): void {
  if (bytes.byteLength > MAX_EPUB_BYTES) {
    throw new Error("EPUB слишком большой: выберите файл до 80 МБ.");
  }
}

export function readZipEntries(bytes: Uint8Array): Map<string, ZipEntry> {
  const view = new DataView(bytes.buffer, bytes.byteOffset, bytes.byteLength);
  const eocdOffset = findEndOfCentralDirectory(view);
  const entryCount = view.getUint16(eocdOffset + 10, true);
  if (entryCount > MAX_ZIP_ENTRIES) {
    throw new Error("EPUB слишком большой: слишком много файлов внутри архива.");
  }
  let offset = view.getUint32(eocdOffset + 16, true);
  const entries = new Map<string, ZipEntry>();
  let totalUncompressedSize = 0;

  for (let index = 0; index < entryCount; index += 1) {
    if (view.getUint32(offset, true) !== 0x02014b50) {
      throw new Error("Некорректный ZIP: повреждён central directory.");
    }

    const method = view.getUint16(offset + 10, true);
    const compressedSize = view.getUint32(offset + 20, true);
    const uncompressedSize = view.getUint32(offset + 24, true);
    const nameLength = view.getUint16(offset + 28, true);
    const extraLength = view.getUint16(offset + 30, true);
    const commentLength = view.getUint16(offset + 32, true);
    const localHeaderOffset = view.getUint32(offset + 42, true);
    const nameStart = offset + 46;
    const name = textDecoder.decode(bytes.subarray(nameStart, nameStart + nameLength));
    totalUncompressedSize += uncompressedSize;
    if (uncompressedSize > MAX_ENTRY_UNCOMPRESSED_BYTES) {
      throw new Error(`EPUB слишком большой: файл ${name} превышает лимит размера.`);
    }
    if (totalUncompressedSize > MAX_TOTAL_UNCOMPRESSED_BYTES) {
      throw new Error("EPUB слишком большой: суммарный распакованный размер превышает лимит.");
    }

    entries.set(name, {
      name,
      method,
      compressedSize,
      uncompressedSize,
      localHeaderOffset,
    });

    offset = nameStart + nameLength + extraLength + commentLength;
  }

  return entries;
}

export async function readZipText(
  bytes: Uint8Array,
  entries: Map<string, ZipEntry>,
  name: string,
): Promise<string> {
  const entry = entries.get(name);
  if (!entry) throw new Error(`В EPUB не найден файл ${name}.`);
  const raw = await readZipEntry(bytes, entry);
  return textDecoder.decode(raw);
}

export async function readZipEntry(bytes: Uint8Array, entry: ZipEntry): Promise<Uint8Array> {
  const view = new DataView(bytes.buffer, bytes.byteOffset, bytes.byteLength);
  const offset = entry.localHeaderOffset;
  if (view.getUint32(offset, true) !== 0x04034b50) {
    throw new Error(`Некорректный ZIP header для ${entry.name}.`);
  }

  const nameLength = view.getUint16(offset + 26, true);
  const extraLength = view.getUint16(offset + 28, true);
  const dataStart = offset + 30 + nameLength + extraLength;
  const compressed = bytes.subarray(dataStart, dataStart + entry.compressedSize);

  if (entry.method === 0) return compressed;
  if (entry.method !== 8) {
    throw new Error(`EPUB использует неподдерживаемое ZIP-сжатие (${entry.method}).`);
  }
  if (!("DecompressionStream" in window)) {
    throw new Error("В этом окружении недоступна распаковка EPUB.");
  }

  const stream = new Blob([compressed])
    .stream()
    .pipeThrough(new DecompressionStream("deflate-raw"));
  const result = new Uint8Array(await new Response(stream).arrayBuffer());
  if (entry.uncompressedSize > 0 && result.length !== entry.uncompressedSize) {
    return result;
  }
  return result;
}

function findEndOfCentralDirectory(view: DataView): number {
  const minOffset = Math.max(0, view.byteLength - 0xffff - 22);
  for (let offset = view.byteLength - 22; offset >= minOffset; offset -= 1) {
    if (view.getUint32(offset, true) === 0x06054b50) return offset;
  }
  throw new Error("Файл не похож на EPUB: ZIP directory не найден.");
}
