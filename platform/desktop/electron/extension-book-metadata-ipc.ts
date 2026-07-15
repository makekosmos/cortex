import { ipcMain, type WebContents } from "electron";
import { fetchBookMetadataPageInBrowser } from "./book-metadata-browser";
import {
  BookMetadataHttpError,
  fetchBookMetadataPage,
  looksLikeBookMetadataBrowserChallenge,
  type BookMetadataPage,
} from "./book-metadata-fetch";
import { keplerLog } from "./logging";
import { lookupOpenLibraryIsbn, type OpenLibraryBookMetadata } from "./book-metadata-open-library";

const CHANNEL = "kepler:extension:book-metadata:fetch-page";
const ISBN_CHANNEL = "kepler:extension:book-metadata:lookup-isbn";

export function registerExtensionBookMetadataIpc(options: {
  assertNetworkRead(sender: WebContents): void;
}): void {
  ipcMain.handle(CHANNEL, async (event, source: unknown): Promise<BookMetadataPage | null> => {
    options.assertNetworkRead(event.sender);
    if (typeof source !== "string" || source.length > 4096) {
      throw new Error("Некорректная ссылка");
    }
    try {
      const page = await fetchBookMetadataPage(source);
      return looksLikeBookMetadataBrowserChallenge(page.html)
        ? await fetchBookMetadataPageInBrowser(source)
        : page;
    } catch (error) {
      if (
        error instanceof BookMetadataHttpError &&
        (error.statusCode === 403 || error.statusCode === 429 || error.statusCode === 503)
      ) {
        return await fetchBookMetadataPageInBrowser(source);
      }
      keplerLog.warn("book-metadata", "page fetch failed", { err: String(error) });
      throw error;
    }
  });

  ipcMain.handle(
    ISBN_CHANNEL,
    async (event, isbn: unknown): Promise<OpenLibraryBookMetadata | null> => {
      options.assertNetworkRead(event.sender);
      if (typeof isbn !== "string") throw new Error("Некорректный ISBN");
      return await lookupOpenLibraryIsbn(isbn);
    },
  );
}
