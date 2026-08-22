import { lookup } from "node:dns/promises";
import type { IncomingMessage } from "node:http";
import { get } from "node:https";
import type { LookupFunction } from "node:net";
import { isPublicNetworkAddress } from "./public-network-address";

export const MAX_BOOK_METADATA_HTML_BYTES = 2 * 1024 * 1024;
const MAX_REDIRECTS = 3;
const ALLOWED_CONTENT_TYPES = new Set(["text/html", "application/xhtml+xml"]);
const BROWSER_USER_AGENT =
  "Mozilla/5.0 (Windows NT 10.0; Win64; x64) AppleWebKit/537.36 " +
  "(KHTML, like Gecko) Chrome/138.0.0.0 Safari/537.36";

type ResolveAddresses = (hostname: string) => Promise<Array<{ address: string; family: number }>>;
type RequestPage = (url: URL, address: string, family: number) => Promise<IncomingMessage>;

export interface BookMetadataPage {
  finalUrl: string;
  html: string;
}

export function looksLikeBookMetadataBrowserChallenge(html: string): boolean {
  const probe = html.slice(0, 100_000).toLowerCase();
  return /ddos-guard|checking your browser|just a moment|провер(?:ка|яем) браузера/.test(probe);
}

export class BookMetadataHttpError extends Error {
  constructor(readonly statusCode: number) {
    super(`Страница вернула HTTP ${statusCode}`);
  }
}

export function parseBookMetadataHttpsUrl(source: string): URL {
  let url: URL;
  try {
    url = new URL(source);
  } catch {
    throw new Error("Некорректная ссылка");
  }
  if (url.protocol !== "https:" || url.username || url.password) {
    throw new Error("Поддерживаются только публичные HTTPS-ссылки");
  }
  return url;
}

function fetchPinned(url: URL, address: string, family: number): Promise<IncomingMessage> {
  return new Promise((resolve, reject) => {
    const pinnedLookup: LookupFunction = (_hostname, options, callback) => {
      if (options.all) callback(null, [{ address, family }]);
      else callback(null, address, family);
    };
    const request = get(
      url,
      {
        headers: {
          Accept: "text/html,application/xhtml+xml",
          "Accept-Language": "ru-RU,ru;q=0.9,en;q=0.7",
          "User-Agent": BROWSER_USER_AGENT,
        },
        lookup: pinnedLookup,
        signal: AbortSignal.timeout(15_000),
      },
      resolve,
    );
    request.once("error", reject);
  });
}

async function readLimitedHtml(response: IncomingMessage): Promise<string> {
  const declaredLength = Number(response.headers["content-length"] ?? 0);
  if (declaredLength > MAX_BOOK_METADATA_HTML_BYTES) {
    response.destroy();
    throw new Error("Страница слишком большая");
  }

  const chunks: Buffer[] = [];
  let total = 0;
  for await (const value of response) {
    const chunk = Buffer.isBuffer(value) ? value : Buffer.from(value);
    total += chunk.byteLength;
    if (total > MAX_BOOK_METADATA_HTML_BYTES) {
      response.destroy();
      throw new Error("Страница слишком большая");
    }
    chunks.push(chunk);
  }
  return Buffer.concat(chunks, total).toString("utf8");
}

export async function fetchBookMetadataPage(
  source: string,
  dependencies: {
    resolveAddresses?: ResolveAddresses;
    requestPage?: RequestPage;
  } = {},
): Promise<BookMetadataPage> {
  let url = parseBookMetadataHttpsUrl(source);
  const resolveAddresses =
    dependencies.resolveAddresses ??
    ((hostname: string) => lookup(hostname, { all: true, verbatim: true }));
  const requestPage = dependencies.requestPage ?? fetchPinned;

  for (let redirects = 0; redirects <= MAX_REDIRECTS; redirects += 1) {
    const addresses = await resolveAddresses(url.hostname);
    if (!addresses.length || addresses.some(({ address }) => !isPublicNetworkAddress(address))) {
      throw new Error("Адрес страницы недоступен");
    }

    const response = await requestPage(url, addresses[0]!.address, addresses[0]!.family);
    const status = response.statusCode ?? 0;
    if (status >= 300 && status < 400) {
      const location = response.headers.location;
      response.destroy();
      if (!location || redirects === MAX_REDIRECTS) {
        throw new Error("Слишком много перенаправлений");
      }
      url = parseBookMetadataHttpsUrl(new URL(location, url).href);
      continue;
    }
    if (status < 200 || status >= 300) {
      response.destroy();
      throw new BookMetadataHttpError(status);
    }

    const contentType = response.headers["content-type"]?.split(";", 1)[0]?.toLowerCase();
    if (!contentType || !ALLOWED_CONTENT_TYPES.has(contentType)) {
      response.destroy();
      throw new Error("Ссылка ведёт не на HTML-страницу");
    }
    return { finalUrl: url.href, html: await readLimitedHtml(response) };
  }

  throw new Error("Слишком много перенаправлений");
}
