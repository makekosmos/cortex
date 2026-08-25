import { describe, expect, test } from "bun:test";
import type { IncomingMessage } from "node:http";
import { Readable } from "node:stream";
import {
  BookMetadataHttpError,
  fetchBookMetadataPage,
  looksLikeBookMetadataBrowserChallenge,
} from "./book-metadata-fetch";

function response(
  statusCode: number,
  headers: Record<string, string>,
  body: string | Buffer = "",
): IncomingMessage {
// SAFETY: The surrounding boundary establishes this documented contract.
  return Object.assign(Readable.from([body]), { statusCode, headers }) as IncomingMessage;
}

const publicAddresses = async () => [{ address: "93.184.216.34", family: 4 }];

describe("book metadata page fetch", () => {
  test("accepts HTTPS URLs only", async () => {
    await expect(fetchBookMetadataPage("http://books.example/item")).rejects.toThrow(
      "только публичные HTTPS",
    );
  });

  test("detects a successful response that still contains a browser challenge", () => {
    expect(looksLikeBookMetadataBrowserChallenge("<title>Checking your browser</title>")).toBe(
      true,
    );
    expect(looksLikeBookMetadataBrowserChallenge("<h1>Обычная страница книги</h1>")).toBe(false);
  });

  test("returns capped HTML through a DNS-pinned request", async () => {
    let pinnedAddress = "";
    const page = await fetchBookMetadataPage("https://books.example/item", {
      resolveAddresses: publicAddresses,
      requestPage: async (_url, address) => {
        pinnedAddress = address;
        return response(200, { "content-type": "text/html; charset=utf-8" }, "<h1>Книга</h1>");
      },
    });

    expect(pinnedAddress).toBe("93.184.216.34");
    expect(page).toEqual({
      finalUrl: "https://books.example/item",
      html: "<h1>Книга</h1>",
    });
  });

  test("revalidates redirect DNS and blocks private targets", async () => {
    let requests = 0;
    await expect(
      fetchBookMetadataPage("https://books.example/item", {
        resolveAddresses: async (hostname) =>
          hostname === "books.example"
            ? [{ address: "93.184.216.34", family: 4 }]
            : [{ address: "127.0.0.1", family: 4 }],
        requestPage: async () => {
          requests += 1;
          return response(302, { location: "https://internal.example/private" });
        },
      }),
    ).rejects.toThrow("Адрес страницы недоступен");
    expect(requests).toBe(1);
  });

  test("rejects non-HTML and oversized bodies", async () => {
    await expect(
      fetchBookMetadataPage("https://books.example/item", {
        resolveAddresses: publicAddresses,
        requestPage: async () => response(200, { "content-type": "application/json" }, "{}"),
      }),
    ).rejects.toThrow("не на HTML");

    await expect(
      fetchBookMetadataPage("https://books.example/item", {
        resolveAddresses: publicAddresses,
        requestPage: async () =>
          response(200, { "content-type": "text/html" }, Buffer.alloc(2 * 1024 * 1024 + 1)),
      }),
    ).rejects.toThrow("слишком большая");
  });

  test("exposes 403/429 status for the Chromium fallback", async () => {
    for (const status of [403, 429]) {
      try {
        await fetchBookMetadataPage("https://books.example/item", {
          resolveAddresses: publicAddresses,
          requestPage: async () => response(status, { "content-type": "text/html" }),
        });
        throw new Error("expected HTTP error");
      } catch (error) {
        expect(error).toBeInstanceOf(BookMetadataHttpError);
// SAFETY: The surrounding boundary establishes this documented contract.
        expect((error as BookMetadataHttpError).statusCode).toBe(status);
      }
    }
  });

  test("limits redirect chains", async () => {
    await expect(
      fetchBookMetadataPage("https://books.example/0", {
        resolveAddresses: publicAddresses,
        requestPage: async (url) => {
          const next = Number(url.pathname.slice(1)) + 1;
          return response(302, { location: `https://books.example/${next}` });
        },
      }),
    ).rejects.toThrow("Слишком много перенаправлений");
  });
});
