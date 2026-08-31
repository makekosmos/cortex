import { mapDiaryEntry } from "./mapping.mjs";
import { dateRange, foodEntries, parseResponse, utcEpochDays, FatSecretError } from "./helpers.mjs";
import { parseFormEncoded, signRequest } from "./oauth1.mjs";

export { FatSecretError } from "./helpers.mjs";

export const FATSECRET_PROVIDER = "fatsecret";
const AUTH_ORIGIN = "https://authentication.fatsecret.com";
const API_ORIGIN = "https://platform.fatsecret.com";
export const FATSECRET_KEYRING_KEYS = Object.freeze({
  consumerSecret: "integrations.fatsecret.consumer_secret",
  accessToken: "integrations.fatsecret.access_token",
  accessTokenSecret: "integrations.fatsecret.access_token_secret",
});

function publicError(kind, message) {
  const safe = String(message ?? "request failed")
    .replace(
      /(["']?authorization["']?\s*[:=]\s*)(?:["']?)(?:bearer|oauth)\b[^}\r\n]*/gi,
      "$1redacted",
    )
    .replace(/\bbearer\s+[a-z0-9._~+/=-]+/gi, "Bearer redacted")
    .replace(
      /(["']?(?:oauth_)?(?:token|secret|key|password|credential|signature)["']?\s*[:=]\s*)(?:"[^"]*"|'[^']*'|[^,}\s]+)/gi,
      "$1redacted",
    )
    .replace(/oauth_[a-z_]+=[^&\s]*/gi, "oauth credential redacted")
    .replace(/(token|secret|key)=?[^&\s]*/gi, "$1 redacted");
  return new FatSecretError(kind, safe);
}

function assertConfig(config) {
  if (
    !config ||
    config.provider !== FATSECRET_PROVIDER ||
    typeof config.clientId !== "string" ||
    !config.clientId ||
    (config.baseUrl !== undefined && typeof config.baseUrl !== "string")
  )
    throw new FatSecretError("config", "FatSecret integration configuration is invalid");
  for (const key of Object.keys(config)) {
    if (/(secret|token|password|credential)/i.test(key))
      throw new FatSecretError("config", "FatSecret secrets must stay in the OS keyring");
  }
}

export class FatSecretIntegration {
  #syncPromise = null;
  #timer = null;
  #connected = false;
  #lastError = null;
  #connectionGeneration = 0;

  constructor({
    config,
    keyring,
    http,
    writer,
    clock = () => Math.floor(Date.now() / 1000),
    random = () => "nonce",
  }) {
    assertConfig(config);
    for (const method of ["get", "set", "delete"]) {
      if (!keyring || typeof keyring[method] !== "function")
        throw new FatSecretError("keyring", "FatSecret requires an OS keyring adapter");
    }
    if (!writer || typeof writer.upsertObject !== "function")
      throw new FatSecretError("ark", "FatSecret requires an ARK upsert operation");
    if (!http || typeof http.request !== "function")
      throw new FatSecretError("transport", "FatSecret requires an injected HTTP transport");
    if (config.baseUrl && new URL(config.baseUrl).origin !== API_ORIGIN)
      throw new FatSecretError("config", "FatSecret URLs must use official FatSecret origins");
    this.config = Object.freeze({ provider: FATSECRET_PROVIDER, clientId: config.clientId });
    this.keyring = keyring;
    this.http = http;
    this.writer = writer;
    this.clock = clock;
    this.random = random;
  }

  status() {
    return {
      provider: FATSECRET_PROVIDER,
      connected: this.#connected,
      syncing: Boolean(this.#syncPromise),
      lastError: this.#lastError,
    };
  }

  async #oauthRequest(method, url, { token, tokenSecret, params = {} } = {}) {
    try {
      const consumerSecret = await this.keyring.get(FATSECRET_KEYRING_KEYS.consumerSecret);
      if (!consumerSecret)
        throw new FatSecretError(
          "keyring",
          "FatSecret consumer secret is unavailable in the OS keyring",
        );
      const requestUrl = new URL(url);
      if (method === "GET")
        for (const [key, value] of Object.entries(params)) requestUrl.searchParams.set(key, value);
      const actualUrl = requestUrl.toString();
      const signed = signRequest({
        method,
        url: actualUrl,
        params: method === "GET" ? {} : params,
        consumerKey: this.config.clientId,
        consumerSecret,
        token,
        tokenSecret,
        oauthVersion: "1.0",
        oauthNonce: this.random(),
        oauthTimestamp: this.clock(),
      });
      const response = await this.http.request({
        method,
        url: actualUrl,
        headers: {
          Authorization: signed.authorization,
          "Content-Type": "application/x-www-form-urlencoded",
        },
        ...(method === "POST" ? { body: new URLSearchParams(params).toString() } : {}),
      });
      return parseFormEncoded(parseResponse(response));
    } catch (error) {
      if (error instanceof FatSecretError) throw error;
      throw publicError("transport", error?.message ?? error);
    }
  }

  async connect({ openBrowser, waitForCallback, callbackUrl }) {
    if (
      typeof openBrowser !== "function" ||
      typeof waitForCallback !== "function" ||
      typeof callbackUrl !== "string" ||
      !callbackUrl
    )
      throw new FatSecretError(
        "oauth",
        "FatSecret OAuth requires browser, callback, and callback URL",
      );
    const generation = this.#connectionGeneration;
    try {
      const request = await this.#oauthRequest("POST", `${AUTH_ORIGIN}/oauth/request_token`, {
        params: { oauth_callback: callbackUrl },
      });
      if (!request.oauth_token || !request.oauth_token_secret)
        throw new FatSecretError("oauth", "FatSecret did not return a request token");
      await openBrowser(
        `${AUTH_ORIGIN}/oauth/authorize?oauth_token=${encodeURIComponent(request.oauth_token)}`,
      );
      const callback = await waitForCallback();
      if (
        callback?.oauthToken !== request.oauth_token ||
        typeof callback.verifier !== "string" ||
        !callback.verifier
      )
        throw new FatSecretError("oauth", "FatSecret OAuth callback is invalid");
      const access = await this.#oauthRequest("GET", `${AUTH_ORIGIN}/oauth/access_token`, {
        token: request.oauth_token,
        tokenSecret: request.oauth_token_secret,
        params: { oauth_verifier: callback.verifier },
      });
      if (!access.oauth_token || !access.oauth_token_secret)
        throw new FatSecretError("oauth", "FatSecret did not return an access token");
      const credentials = [
        [FATSECRET_KEYRING_KEYS.accessToken, access.oauth_token],
        [FATSECRET_KEYRING_KEYS.accessTokenSecret, access.oauth_token_secret],
      ];
      try {
        for (const [key, value] of credentials) {
          if (generation !== this.#connectionGeneration) break;
          await this.keyring.set(key, value);
        }
        if (generation !== this.#connectionGeneration)
          throw new FatSecretError("oauth", "FatSecret connection was cancelled");
      } catch (error) {
        await Promise.all(credentials.map(([key]) => this.keyring.delete(key)));
        throw error;
      }
      this.#connected = true;
      this.#lastError = null;
      return this.status();
    } catch (error) {
      const safe =
        error instanceof FatSecretError ? error : publicError("oauth", error?.message ?? error);
      this.#lastError = { kind: safe.kind, message: safe.message };
      throw safe;
    }
  }

  async syncNow({ from, to } = {}) {
    if (this.#syncPromise) return this.#syncPromise;
    this.#syncPromise = this.#sync({ from, to }).finally(() => {
      this.#syncPromise = null;
    });
    return this.#syncPromise;
  }

  async #sync({ from, to }) {
    const generation = this.#connectionGeneration;
    try {
      const token = await this.keyring.get(FATSECRET_KEYRING_KEYS.accessToken);
      const tokenSecret = await this.keyring.get(FATSECRET_KEYRING_KEYS.accessTokenSecret);
      const consumerSecret = await this.keyring.get(FATSECRET_KEYRING_KEYS.consumerSecret);
      if (!token || !tokenSecret || !consumerSecret)
        throw new FatSecretError("auth", "Connect FatSecret before syncing");
      let imported = 0;
      for (const day of dateRange(from, to)) {
        const requestUrl = new URL(`${API_ORIGIN}/rest/food-entries/v2`);
        requestUrl.searchParams.set("date", String(utcEpochDays(day)));
        requestUrl.searchParams.set("format", "json");
        const signed = signRequest({
          method: "GET",
          url: requestUrl.toString(),
          consumerKey: this.config.clientId,
          consumerSecret,
          token,
          tokenSecret,
          oauthVersion: "1.0",
          oauthNonce: this.random(),
          oauthTimestamp: this.clock(),
        });
        let response;
        try {
          response = await this.http.request({
            method: "GET",
            url: requestUrl.toString(),
            headers: { Authorization: signed.authorization },
          });
        } catch (error) {
          throw publicError("transport", error?.message ?? error);
        }
        const entries = foodEntries(parseResponse(response));
        for (const entry of entries) await this.writer.upsertObject(mapDiaryEntry(entry));
        imported += entries.length;
      }
      if (typeof this.writer.recordSync === "function")
        await this.writer.recordSync({
          provider: FATSECRET_PROVIDER,
          from,
          to,
          imported,
          completedAt: new Date(this.clock() * 1000).toISOString(),
        });
      if (generation === this.#connectionGeneration) this.#connected = true;
      this.#lastError = null;
      return { imported, from, to };
    } catch (error) {
      const safe =
        error instanceof FatSecretError
          ? error
          : publicError("invalid_response", error?.message ?? error);
      this.#lastError = { kind: safe.kind, message: safe.message };
      throw safe;
    }
  }

  async start({ intervalMs = 24 * 60 * 60 * 1000, from, to } = {}) {
    if (this.#timer) return this.status();
    await this.syncNow({ from, to }).catch(() => {});
    this.#timer = setInterval(() => void this.syncNow({ from, to }).catch(() => {}), intervalMs);
    return this.status();
  }

  stop() {
    if (this.#timer) clearInterval(this.#timer);
    this.#timer = null;
    return this.status();
  }

  async disconnect() {
    this.#connectionGeneration += 1;
    this.#connected = false;
    this.stop();
    for (const key of [
      FATSECRET_KEYRING_KEYS.accessToken,
      FATSECRET_KEYRING_KEYS.accessTokenSecret,
      FATSECRET_KEYRING_KEYS.consumerSecret,
    ])
      await this.keyring.delete(key);
    return this.status();
  }
}
