import { createHmac, randomBytes } from "node:crypto";

export function encode(value) {
  return encodeURIComponent(String(value)).replace(
    /[!'()*]/g,
    (character) => `%${character.charCodeAt(0).toString(16).toUpperCase()}`,
  );
}

export function parseFormEncoded(value) {
  const result = new Map();
  for (const pair of String(value ?? "").split("&")) {
    if (!pair) continue;
    const [rawKey, rawValue = ""] = pair.split("=", 2);
    const key = decodeURIComponent(rawKey.replace(/\+/g, " "));
    result.set(key, decodeURIComponent(rawValue.replace(/\+/g, " ")));
  }
  return Object.fromEntries(result);
}

export function parseOAuthHeader(header) {
  const value = String(header ?? "").replace(/^OAuth\s+/i, "");
  const result = {};
  for (const match of value.matchAll(/(?:^|,)\s*([^=\s]+)="((?:\\.|[^"])*)"/g)) {
    result[decodeURIComponent(match[1])] = decodeURIComponent(match[2].replace(/\\(["\\])/g, "$1"));
  }
  return result;
}

function baseUrl(input) {
  const url = new URL(input);
  url.hash = "";
  url.search = "";
  return url.toString().replace(/\/$/, "");
}

function collectParameters(url, params) {
  const values = [];
  const parsed = new URL(url);
  for (const [key, value] of parsed.searchParams) values.push([key, value]);
  for (const [key, value] of Object.entries(params ?? {})) {
    if (value === undefined || value === null) continue;
    if (Array.isArray(value)) for (const item of value) values.push([key, item]);
    else values.push([key, value]);
  }
  return values;
}

export function normalizedParameters(url, params) {
  return collectParameters(url, params)
    .map(([key, value]) => [encode(key), encode(value)])
    .sort(
      ([aKey, aValue], [bKey, bValue]) => aKey.localeCompare(bKey) || aValue.localeCompare(bValue),
    )
    .map(([key, value]) => `${key}=${value}`)
    .join("&");
}

export function signatureBaseString({ method, url, params }) {
  return [
    String(method).toUpperCase(),
    encode(baseUrl(url)),
    encode(normalizedParameters(url, params)),
  ].join("&");
}

function nonce() {
  return randomBytes(16).toString("hex");
}

export function signRequest({
  method,
  url,
  params = {},
  consumerKey,
  consumerSecret,
  token,
  tokenSecret,
  oauthNonce = nonce(),
  oauthTimestamp = Math.floor(Date.now() / 1000),
  oauthVersion,
}) {
  if (!consumerKey || !consumerSecret) throw new Error("OAuth consumer credentials are required");
  const oauth = {
    oauth_consumer_key: consumerKey,
    oauth_nonce: oauthNonce,
    oauth_signature_method: "HMAC-SHA1",
    oauth_timestamp: String(oauthTimestamp),
  };
  if (oauthVersion) oauth.oauth_version = oauthVersion;
  if (token) oauth.oauth_token = token;
  const base = signatureBaseString({ method, url, params: { ...params, ...oauth } });
  const signingKey = `${encode(consumerSecret)}&${encode(tokenSecret ?? "")}`;
  const signature = createHmac("sha1", signingKey).update(base).digest("base64");
  const header =
    "OAuth " +
    Object.entries({ ...oauth, oauth_signature: signature })
      .sort(([a], [b]) => a.localeCompare(b))
      .map(([key, value]) => `${encode(key)}="${encode(value)}"`)
      .join(", ");
  return { authorization: header, signature, oauth, baseString: base };
}
