import {
  assertEpubSize,
  readZipEntries,
  readZipEntry,
  readZipText,
  type ZipEntry,
} from "./epubZip";
import { joinZipPath, parentDir, titleFromHref } from "./epubPath";

export interface EpubBook {
  title: string;
  authors: string[];
  coverDataUrl: string | null;
  chapters: EpubChapter[];
  blocks: ReaderBlock[];
}

export interface EpubChapter {
  id: string;
  title: string;
  href: string;
  startBlock: number;
}

export interface ReaderBlock {
  id: string;
  chapterId: string;
  kind: "heading" | "paragraph" | "list-item" | "blockquote";
  level: number;
  spans: InlineSpan[];
}

interface InlineSpan {
  text: string;
  strong: boolean;
  emphasis: boolean;
}

interface PackageManifestItem {
  id: string;
  href: string;
  mediaType: string;
  properties: string;
}

const MAX_COVER_BYTES = 8 * 1024 * 1024;
const MAX_READER_BLOCKS = 60_000;

export async function readEpubBytes(bytes: Uint8Array, fallbackName: string): Promise<EpubBook> {
  assertEpubSize(bytes);
  const entries = readZipEntries(bytes);
  const container = await readZipText(bytes, entries, "META-INF/container.xml");
  const rootfile = parseRootfilePath(container);
  const opf = await readZipText(bytes, entries, rootfile);
  const parsedPackage = parsePackage(opf);
  const base = parentDir(rootfile);
  const coverDataUrl = await readCoverDataUrl(bytes, entries, parsedPackage, base);

  const blocks: ReaderBlock[] = [];
  const chapters: EpubChapter[] = [];

  for (const spineItemId of parsedPackage.spine) {
    const item = parsedPackage.manifest.get(spineItemId);
    if (!item || !item.mediaType.includes("xhtml")) continue;

    const href = joinZipPath(base, item.href);
    const xhtml = await readZipText(bytes, entries, href);
    const chapterId = item.id;
    const chapterBlocks = parseXhtmlChapter(xhtml, chapterId);
    if (blocks.length + chapterBlocks.length > MAX_READER_BLOCKS) {
      throw new Error("EPUB слишком большой: слишком много текстовых блоков.");
    }
    const fallbackTitle = titleFromHref(item.href);
    const title = chapterBlocks
      .find((block) => block.kind === "heading")
      ?.spans.map((span) => span.text)
      .join("")
      .trim();

    chapters.push({
      id: chapterId,
      title: title || fallbackTitle,
      href,
      startBlock: blocks.length,
    });
    blocks.push(...chapterBlocks);
  }

  return {
    title: parsedPackage.title || fallbackName.replace(/\.epub$/i, ""),
    authors: parsedPackage.authors,
    coverDataUrl,
    chapters,
    blocks,
  };
}

function parseRootfilePath(container: string): string {
  const doc = parseXml(container);
  const rootfile = doc.querySelector("rootfile[full-path]");
  const path = rootfile?.getAttribute("full-path");
  if (!path) throw new Error("EPUB container.xml не содержит rootfile.");
  return path;
}

function parsePackage(opf: string): {
  title: string;
  authors: string[];
  coverId: string | null;
  manifest: Map<string, PackageManifestItem>;
  spine: string[];
} {
  const doc = parseXml(opf);
  const title =
    Array.from(doc.getElementsByTagName("*"))
      .find((node) => node.localName === "title")
      ?.textContent?.trim() ?? "";
  const authors = Array.from(doc.getElementsByTagName("*"))
    .filter((node) => node.localName === "creator")
    .map((node) => node.textContent?.trim() ?? "")
    .filter(Boolean);
  const coverId =
    Array.from(doc.getElementsByTagName("*"))
      .find((node) => node.localName === "meta" && node.getAttribute("name") === "cover")
      ?.getAttribute("content") ?? null;
  const manifest = new Map<string, PackageManifestItem>();
  const spine: string[] = [];

  for (const item of Array.from(doc.querySelectorAll("manifest item"))) {
    const id = item.getAttribute("id");
    const href = item.getAttribute("href");
    const mediaType = item.getAttribute("media-type") ?? "";
    const properties = item.getAttribute("properties") ?? "";
    if (id && href) manifest.set(id, { id, href, mediaType, properties });
  }

  for (const itemref of Array.from(doc.querySelectorAll("spine itemref"))) {
    const idref = itemref.getAttribute("idref");
    if (idref) spine.push(idref);
  }

  return { title, authors, coverId, manifest, spine };
}

async function readCoverDataUrl(
  bytes: Uint8Array,
  entries: Map<string, ZipEntry>,
  parsedPackage: {
    coverId: string | null;
    manifest: Map<string, PackageManifestItem>;
  },
  base: string,
): Promise<string | null> {
  const coverItem =
    Array.from(parsedPackage.manifest.values()).find((item) =>
      item.properties.split(/\s+/).includes("cover-image"),
    ) ?? (parsedPackage.coverId ? parsedPackage.manifest.get(parsedPackage.coverId) : undefined);

  if (!coverItem || !coverItem.mediaType.startsWith("image/")) return null;
  const href = joinZipPath(base, coverItem.href);
  const entry = entries.get(href);
  if (!entry) return null;
  if (entry.uncompressedSize > MAX_COVER_BYTES) return null;
  const data = await readZipEntry(bytes, entry);
  return `data:${coverItem.mediaType};base64,${uint8ToBase64(data)}`;
}

function uint8ToBase64(bytes: Uint8Array): string {
  let binary = "";
  const chunkSize = 0x8000;
  for (let index = 0; index < bytes.length; index += chunkSize) {
    binary += String.fromCharCode(...bytes.subarray(index, index + chunkSize));
  }
  return btoa(binary);
}

function parseXhtmlChapter(xhtml: string, chapterId: string): ReaderBlock[] {
  const doc = parseXml(xhtml);
  const root = doc.body ?? doc.documentElement;
  const blocks: ReaderBlock[] = [];
  let index = 0;

  function visit(element: Element) {
    const kind = blockKind(element);
    if (kind) {
      const spans = collectInlineSpans(element);
      const plain = spans
        .map((span) => span.text)
        .join("")
        .trim();
      if (plain) {
        blocks.push({
          id: `${chapterId}-${index}`,
          chapterId,
          kind: kind.kind,
          level: kind.level,
          spans,
        });
        index += 1;
      }
      return;
    }

    for (const child of Array.from(element.children)) visit(child);
  }

  visit(root);
  return blocks;
}

function blockKind(element: Element): { kind: ReaderBlock["kind"]; level: number } | null {
  const tag = element.localName.toLowerCase();
  if (/^h[1-6]$/.test(tag)) return { kind: "heading", level: Number(tag.slice(1)) };
  if (tag === "p") return { kind: "paragraph", level: 0 };
  if (tag === "li") return { kind: "list-item", level: 0 };
  if (tag === "blockquote") return { kind: "blockquote", level: 0 };
  return null;
}

function collectInlineSpans(root: Node): InlineSpan[] {
  const spans: InlineSpan[] = [];

  function walk(node: Node, strong: boolean, emphasis: boolean) {
    if (node.nodeType === Node.TEXT_NODE) {
      pushText(spans, node.textContent ?? "", strong, emphasis);
      return;
    }
    if (node.nodeType !== Node.ELEMENT_NODE) return;

    const element = node as Element;
    const tag = element.localName.toLowerCase();
    const nextStrong = strong || ["strong", "b"].includes(tag) || isBridgehead(element);
    const nextEmphasis = emphasis || ["em", "i", "cite"].includes(tag);

    if (tag === "br") pushText(spans, "\n", nextStrong, nextEmphasis);
    for (const child of Array.from(element.childNodes)) walk(child, nextStrong, nextEmphasis);
  }

  walk(root, false, false);
  return trimSpans(spans);
}

function pushText(spans: InlineSpan[], value: string, strong: boolean, emphasis: boolean) {
  const normalized = value.replace(/\s+/g, " ");
  if (!normalized.trim()) {
    if (spans.length > 0 && !spans.at(-1)?.text.endsWith(" ")) {
      spans[spans.length - 1].text += " ";
    }
    return;
  }

  const previous = spans.at(-1);
  const text =
    previous && shouldInsertSpace(previous.text, normalized)
      ? ` ${normalized.trimStart()}`
      : normalized;

  if (previous && previous.strong === strong && previous.emphasis === emphasis) {
    previous.text += text;
    return;
  }
  spans.push({ text, strong, emphasis });
}

function trimSpans(spans: InlineSpan[]): InlineSpan[] {
  const first = spans[0];
  const last = spans.at(-1);
  if (first) first.text = first.text.trimStart();
  if (last) last.text = last.text.trimEnd();
  return spans.filter((span) => span.text.length > 0);
}

function shouldInsertSpace(previous: string, next: string): boolean {
  if (!previous || previous.endsWith(" ") || next.startsWith(" ")) return false;
  return /[\p{L}\p{N})\]]/u.test(previous.at(-1) ?? "") && /[\p{L}\p{N}([]/u.test(next[0] ?? "");
}

function isBridgehead(element: Element): boolean {
  return element.getAttribute("epub:type") === "bridgehead";
}

function parseXml(source: string): Document {
  const doc = new DOMParser().parseFromString(source, "application/xml");
  const error = doc.querySelector("parsererror");
  if (error) throw new Error("EPUB содержит некорректный XML/XHTML.");
  return doc;
}
