const OPEN_LIBRARY_ORIGIN = "https://openlibrary.org";
const MAX_RESPONSE_BYTES = 256 * 1024;
const MAX_AUTHORS = 4;
const MAX_REDIRECTS = 3;
import {
  isNumber,
  isRecord,
  isString,
  parseJsonRecord,
  type JsonRecord,
  type JsonValue,
} from "./json-contracts";

export interface OpenLibraryBookMetadata {
  title?: string;
  author?: string;
  cover_image?: string;
  isbn?: string;
  page_count?: number;
  language?: string;
  publisher?: string;
  published_date?: string;
}

type Fetcher = typeof fetch;
function isValidIsbn13(value: string): boolean {
  if (!/^\d{13}$/.test(value)) return false;
  const sum = value
    .slice(0, 12)
    .split("")
    .reduce((total, character, index) => total + Number(character) * (index % 2 === 0 ? 1 : 3), 0);
  return (10 - (sum % 10)) % 10 === Number(value[12]);
}

async function readLimitedText(response: Response): Promise<string> {
  const declaredLength = Number(response.headers.get("content-length") ?? 0);
  if (declaredLength > MAX_RESPONSE_BYTES) throw new Error("Ответ Open Library слишком большой");

  const reader = response.body?.getReader();
  if (!reader) return "";
  const chunks: Uint8Array[] = [];
  let total = 0;
  for (;;) {
    const { done, value } = await reader.read();
    if (done) break;
    total += value.byteLength;
    if (total > MAX_RESPONSE_BYTES) {
      await reader.cancel();
      throw new Error("Ответ Open Library слишком большой");
    }
    chunks.push(value);
  }

  const bytes = new Uint8Array(total);
  let offset = 0;
  for (const chunk of chunks) {
    bytes.set(chunk, offset);
    offset += chunk.byteLength;
  }
  return new TextDecoder().decode(bytes);
}

async function fetchJson(path: string, fetcher: Fetcher): Promise<JsonRecord | null> {
  let url = new URL(path, OPEN_LIBRARY_ORIGIN);
  if (url.origin !== OPEN_LIBRARY_ORIGIN) throw new Error("Некорректный путь Open Library");
  for (let redirectCount = 0; ; redirectCount += 1) {
    const response = await fetcher(url, {
      headers: { Accept: "application/json", "User-Agent": "Kosmos Eden/0.5" },
      redirect: "manual",
      signal: AbortSignal.timeout(10_000),
    });
    if (response.status >= 300 && response.status < 400) {
      if (redirectCount >= MAX_REDIRECTS) throw new Error("Слишком много редиректов Open Library");
      const location = response.headers.get("location");
      if (!location) throw new Error("Open Library вернул редирект без адреса");
      const nextUrl = new URL(location, url);
      if (nextUrl.origin !== OPEN_LIBRARY_ORIGIN) {
        throw new Error("Open Library перенаправил на внешний адрес");
      }
      url = nextUrl;
      continue;
    }
    if (response.status === 404) return null;
    if (!response.ok) throw new Error(`Open Library вернул HTTP ${response.status}`);
    const contentType = response.headers.get("content-type")?.toLowerCase() ?? "";
    if (!contentType.includes("application/json")) throw new Error("Open Library вернул не JSON");
    return parseJsonRecord(JSON.parse(await readLimitedText(response)));
  }
}

function text(value: JsonValue | undefined): string {
  return isString(value) ? value.trim() : "";
}

function stringList(value: JsonValue | undefined): string {
  return Array.isArray(value) ? value.map(text).filter(Boolean).join(", ") : "";
}

function languageList(value: JsonValue | undefined): string {
  if (!Array.isArray(value)) return "";
  return value
    .map((item) => (isRecord(item) ? text(item.key) : ""))
    .map((key) => key.split("/").at(-1) ?? "")
    .filter(Boolean)
    .join(", ");
}

async function authorNames(value: JsonValue | undefined, fetcher: Fetcher): Promise<string> {
  if (!Array.isArray(value)) return "";
  const keys = value
    .map((item) => (isRecord(item) ? text(item.key) : ""))
    .filter((key) => /^\/authors\/OL\d+A$/.test(key))
    .slice(0, MAX_AUTHORS);
  const authors = await Promise.all(
    keys.map((key) => fetchJson(`${key}.json`, fetcher).catch(() => null)),
  );
  return authors
    .map((author) => text(author?.name))
    .filter(Boolean)
    .join(", ");
}

export async function lookupOpenLibraryIsbn(
  isbn: string,
  options: { fetcher?: Fetcher } = {},
): Promise<OpenLibraryBookMetadata | null> {
  if (!isValidIsbn13(isbn)) throw new Error("Некорректный ISBN");
  const fetcher = options.fetcher ?? fetch;
  const edition = await fetchJson(`/isbn/${isbn}.json`, fetcher);
  if (!edition) return null;

  const title = text(edition.title);
  const author = await authorNames(edition.authors, fetcher);
  const publisher = stringList(edition.publishers);
  const publishedDate = text(edition.publish_date);
  const language = languageList(edition.languages);
  const pageCount =
    isNumber(edition.number_of_pages) && edition.number_of_pages > 0
      ? edition.number_of_pages
      : undefined;
  const coverId = Array.isArray(edition.covers)
    ? edition.covers.find((value) => isNumber(value) && value > 0)
    : undefined;

  return {
    ...(title && { title }),
    ...(author && { author }),
    ...(coverId && {
      cover_image: `https://covers.openlibrary.org/b/id/${coverId}-L.jpg?default=false`,
    }),
    isbn,
    ...(pageCount && { page_count: pageCount }),
    ...(language && { language }),
    ...(publisher && { publisher }),
    ...(publishedDate && { published_date: publishedDate }),
  };
}
