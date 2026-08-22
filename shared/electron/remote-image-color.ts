import { nativeImage } from "electron";
import { lookup } from "node:dns/promises";
import type { IncomingMessage } from "node:http";
import { get } from "node:https";
import type { LookupFunction } from "node:net";
import { dominantImageColor } from "./image-dominant-color";
import { imageDimensions } from "./image-dimensions";
import { isPublicNetworkAddress } from "./public-network-address";

const MAX_IMAGE_BYTES = 10 * 1024 * 1024;
const ALLOWED_IMAGE_TYPES = new Set(["image/jpeg", "image/png"]);

async function readLimitedImage(response: IncomingMessage): Promise<Buffer | null> {
  const chunks: Buffer[] = [];
  let total = 0;
  for await (const value of response) {
    const chunk = Buffer.isBuffer(value) ? value : Buffer.from(value);
    total += chunk.byteLength;
    if (total > MAX_IMAGE_BYTES) {
      response.destroy();
      return null;
    }
    chunks.push(chunk);
  }
  return Buffer.concat(chunks, total);
}

function fetchPinned(url: URL, address: string, family: number): Promise<IncomingMessage> {
  return new Promise((resolve, reject) => {
    const pinnedLookup: LookupFunction = (_hostname, options, callback) => {
      if (options.all) callback(null, [{ address, family }]);
      else callback(null, address, family);
    };
    const request = get(
      url,
      { lookup: pinnedLookup, signal: AbortSignal.timeout(10_000) },
      resolve,
    );
    request.once("error", reject);
  });
}

export async function dominantRemoteImageColor(source: string): Promise<string | null> {
  let url: URL;
  try {
    url = new URL(source);
  } catch {
    return null;
  }
  if (url.protocol !== "https:" || url.username || url.password) return null;

  let response: IncomingMessage | null = null;
  for (let redirects = 0; redirects <= 3; redirects += 1) {
    const addresses = await lookup(url.hostname, { all: true, verbatim: true });
    if (!addresses.length || addresses.some(({ address }) => !isPublicNetworkAddress(address)))
      return null;
    response = await fetchPinned(url, addresses[0]!.address, addresses[0]!.family);
    const status = response.statusCode ?? 0;
    if (status < 300 || status >= 400) break;
    const location = response.headers.location;
    response.destroy();
    if (!location || redirects === 3) return null;
    url = new URL(location, url);
    if (url.protocol !== "https:" || url.username || url.password) return null;
  }
  const status = response?.statusCode ?? 0;
  const contentType = response?.headers["content-type"]?.split(";", 1)[0]?.toLowerCase();
  if (
    !response ||
    status < 200 ||
    status >= 300 ||
    !contentType ||
    !ALLOWED_IMAGE_TYPES.has(contentType)
  ) {
    response?.destroy();
    return null;
  }
  const declaredLength = Number(response.headers["content-length"] ?? 0);
  if (declaredLength > MAX_IMAGE_BYTES) {
    response.destroy();
    return null;
  }
  const bytes = await readLimitedImage(response);
  if (!bytes) return null;
  const dimensions = imageDimensions(bytes);
  if (!dimensions || dimensions.width * dimensions.height > 40_000_000) return null;
  const image = nativeImage.createFromBuffer(bytes);
  if (image.isEmpty()) return null;
  const bitmap = image.resize({ width: 32, height: 32, quality: "good" }).toBitmap();
  const rgba = new Uint8Array(bitmap.length);
  for (let index = 0; index < bitmap.length; index += 4) {
    rgba[index] = bitmap[index + 2];
    rgba[index + 1] = bitmap[index + 1];
    rgba[index + 2] = bitmap[index];
    rgba[index + 3] = bitmap[index + 3];
  }
  return dominantImageColor(rgba, 4);
}
