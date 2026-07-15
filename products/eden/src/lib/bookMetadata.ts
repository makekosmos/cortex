import { normalizeBookLanguage } from "./bookLanguages";

export const BOOK_METADATA_FIELD_IDS = [
  "author",
  "cover_image",
  "isbn",
  "page_count",
  "language",
  "publisher",
  "published_date",
  "source_url",
] as const;

export type BookMetadataFieldId = (typeof BOOK_METADATA_FIELD_IDS)[number];

export interface BookMetadata {
  title?: string;
  author?: string;
  cover_image?: string;
  isbn?: string;
  page_count?: number;
  language?: string;
  publisher?: string;
  published_date?: string;
  source_url?: string;
}

export interface BookMetadataPage {
  finalUrl: string;
  html: string;
}

type JsonRecord = Record<string, unknown>;

const BOOK_TYPES = new Set(["book", "audiobook", "publicationvolume", "product"]);

function cleanText(value: unknown): string {
  if (typeof value === "number" && Number.isFinite(value)) return String(value);
  if (typeof value !== "string") return "";
  return value.replace(/\s+/g, " ").trim();
}

function firstText(...values: unknown[]): string {
  for (const value of values) {
    const text = cleanText(value);
    if (text) return text;
  }
  return "";
}

function stringFromJsonValue(value: unknown): string {
  if (Array.isArray(value)) {
    return value.map(stringFromJsonValue).filter(Boolean).join(", ");
  }
  if (value && typeof value === "object") {
    const record = value as JsonRecord;
    return firstText(record.name, record["@value"], record.url, record.contentUrl);
  }
  return cleanText(value);
}

function collectJsonRecords(value: unknown, records: JsonRecord[]): void {
  if (Array.isArray(value)) {
    for (const item of value) collectJsonRecords(item, records);
    return;
  }
  if (!value || typeof value !== "object") return;
  const record = value as JsonRecord;
  records.push(record);
  if (record["@graph"]) collectJsonRecords(record["@graph"], records);
  if (record.mainEntity) collectJsonRecords(record.mainEntity, records);
  if (record.item) collectJsonRecords(record.item, records);
}

function jsonTypeMatches(record: JsonRecord): boolean {
  const raw = record["@type"];
  const values = Array.isArray(raw) ? raw : [raw];
  return values.some((value) => BOOK_TYPES.has(cleanText(value).toLowerCase()));
}

function parseJsonLd(document: Document): JsonRecord[] {
  const records: JsonRecord[] = [];
  for (const script of document.querySelectorAll('script[type="application/ld+json"]')) {
    try {
      collectJsonRecords(JSON.parse(script.textContent ?? ""), records);
    } catch {
      // A malformed block must not hide valid metadata from the rest of the page.
    }
  }
  return records.filter(jsonTypeMatches);
}

function meta(document: Document, ...names: string[]): string {
  for (const name of names) {
    const escaped = CSS.escape(name);
    const element = document.querySelector<HTMLMetaElement>(
      `meta[property="${escaped}"], meta[name="${escaped}"], meta[itemprop="${escaped}"]`,
    );
    const value = cleanText(element?.content);
    if (value) return value;
  }
  return "";
}

function elementValue(document: Document, ...selectors: string[]): string {
  for (const selector of selectors) {
    const element = document.querySelector<HTMLElement>(selector);
    const value = firstText(
      element?.getAttribute("content"),
      element?.getAttribute("href"),
      element?.getAttribute("src"),
      element?.textContent,
    );
    if (value) return value;
  }
  return "";
}

function pageLines(document: Document): string[] {
  const elements = document.body?.querySelectorAll(
    "h1, h2, h3, p, li, dt, dd, th, td, div, section",
  );
  if (!elements) return [];
  const lines: string[] = [];
  let previous = "";
  for (const element of elements) {
    const text = cleanText(element.textContent);
    if (!text || text === previous || text.length > 500) continue;
    lines.push(text);
    previous = text;
  }
  return lines;
}

function labeledValue(lines: string[], labels: RegExp): string {
  for (let index = 0; index < lines.length; index += 1) {
    const line = lines[index] ?? "";
    const match = line.match(labels);
    if (!match) continue;
    const inline = cleanText(match[1]);
    if (inline) return inline;
    return cleanText(lines[index + 1]);
  }
  return "";
}

function absoluteHttpUrl(value: string, baseUrl: string): string {
  if (!value) return "";
  try {
    const url = new URL(value, baseUrl);
    return url.protocol === "http:" || url.protocol === "https:" ? url.href : "";
  } catch {
    return "";
  }
}

function validIsbn10(value: string): boolean {
  if (!/^\d{9}[\dX]$/.test(value)) return false;
  const sum = [...value].reduce(
    (total, character, index) =>
      total + (character === "X" ? 10 : Number(character)) * (10 - index),
    0,
  );
  return sum % 11 === 0;
}

function validIsbn13(value: string): boolean {
  if (!/^\d{13}$/.test(value)) return false;
  const sum = value
    .slice(0, 12)
    .split("")
    .reduce((total, character, index) => total + Number(character) * (index % 2 === 0 ? 1 : 3), 0);
  return (10 - (sum % 10)) % 10 === Number(value[12]);
}

function isbn10To13(value: string): string {
  const firstTwelve = `978${value.slice(0, 9)}`;
  const sum = [...firstTwelve].reduce(
    (total, character, index) => total + Number(character) * (index % 2 === 0 ? 1 : 3),
    0,
  );
  return `${firstTwelve}${(10 - (sum % 10)) % 10}`;
}

export function normalizeIsbn(value: unknown): string {
  const candidates =
    cleanText(value)
      .toUpperCase()
      .match(/(?:97[89][\d\s-]{10,20}|[\dX][\dX\s-]{8,18})/g) ?? [];
  for (const candidate of candidates) {
    const normalized = candidate.replace(/[^\dX]/g, "");
    if (validIsbn13(normalized)) return normalized;
    if (validIsbn10(normalized)) return isbn10To13(normalized);
  }
  return "";
}

function pageCount(value: unknown): number | undefined {
  const match = cleanText(value).match(/\d{1,6}/);
  if (!match) return undefined;
  const number = Number(match[0]);
  return number > 0 && number <= 100_000 ? number : undefined;
}

function firstJsonValue(records: JsonRecord[], ...keys: string[]): unknown {
  for (const record of records) {
    for (const key of keys) {
      if (record[key] !== undefined && stringFromJsonValue(record[key])) return record[key];
    }
  }
  return undefined;
}

export function extractBookMetadata(page: BookMetadataPage): BookMetadata {
  const document = new DOMParser().parseFromString(page.html, "text/html");
  const records = parseJsonLd(document);
  const lines = pageLines(document);

  const title = firstText(
    stringFromJsonValue(firstJsonValue(records, "name", "headline")),
    elementValue(document, "main h1", "article h1", "h1"),
    meta(document, "og:title", "twitter:title"),
  );
  const author = firstText(
    stringFromJsonValue(firstJsonValue(records, "author", "creator")),
    elementValue(
      document,
      '[itemprop="author"]',
      'a[rel="author"]',
      "main h1 + a",
      "main h1 + div a",
      "article h1 + a",
    ),
    meta(document, "author", "book:author"),
  );
  const rawCover = firstText(
    stringFromJsonValue(firstJsonValue(records, "image", "thumbnailUrl", "primaryImageOfPage")),
    meta(document, "og:image", "twitter:image", "twitter:image:src"),
    elementValue(document, '[itemprop="image"]'),
  );
  const rawIsbn = firstText(
    stringFromJsonValue(firstJsonValue(records, "isbn")),
    meta(document, "book:isbn", "isbn"),
    labeledValue(lines, /^ISBN\s*[:：]?\s*(.*)$/i),
  );
  const rawPages = firstText(
    stringFromJsonValue(firstJsonValue(records, "numberOfPages", "pageCount")),
    meta(document, "book:page_count", "numberOfPages"),
    labeledValue(lines, /^(?:Количество страниц|Страниц|Pages)\s*[:：]?\s*(.*)$/i),
  );
  const language = normalizeBookLanguage(
    firstText(
      stringFromJsonValue(firstJsonValue(records, "inLanguage", "language")),
      meta(document, "book:language", "language"),
      labeledValue(lines, /^(?:Язык|Language)\s*[:：]?\s*(.*)$/i),
    ),
  );
  const publisher = firstText(
    stringFromJsonValue(firstJsonValue(records, "publisher")),
    meta(document, "book:publisher", "publisher"),
    labeledValue(lines, /^(?:Издательство|Publisher)\s*[:：]?\s*(.*)$/i),
  );
  const publishedDate = firstText(
    stringFromJsonValue(firstJsonValue(records, "datePublished", "copyrightYear")),
    meta(document, "book:release_date", "datePublished"),
    labeledValue(lines, /^(?:Год издания|Дата издания|Published)\s*[:：]?\s*(.*)$/i),
  );
  const isbn = normalizeIsbn(rawIsbn);
  const pages = pageCount(rawPages);

  return {
    ...(title && { title }),
    ...(author && { author }),
    ...(rawCover && { cover_image: absoluteHttpUrl(rawCover, page.finalUrl) }),
    ...(isbn && { isbn }),
    ...(pages && { page_count: pages }),
    ...(language && { language }),
    ...(publisher && { publisher }),
    ...(publishedDate && { published_date: publishedDate }),
    source_url: page.finalUrl,
  };
}

export function hasExtractedBookData(metadata: BookMetadata): boolean {
  return Boolean(
    metadata.title ||
    metadata.author ||
    metadata.cover_image ||
    metadata.isbn ||
    metadata.page_count ||
    metadata.language ||
    metadata.publisher ||
    metadata.published_date,
  );
}

export function isEmptyBookValue(value: unknown): boolean {
  if (typeof value === "number") return !Number.isFinite(value);
  if (Array.isArray(value)) return value.length === 0;
  return value === null || value === undefined || cleanText(value) === "";
}

export function fillMissingBookMetadata(
  primary: BookMetadata,
  fallback: BookMetadata,
): BookMetadata {
  const merged = { ...fallback };
  for (const [key, value] of Object.entries(primary)) {
    if (!isEmptyBookValue(value)) merged[key as keyof BookMetadata] = value as never;
  }
  return merged;
}
