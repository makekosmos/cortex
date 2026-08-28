import { mapDiaryEntry } from "./mapping.mjs";
import { parseFormEncoded, signRequest } from "./oauth1.mjs";

export const FATSECRET_PROVIDER = "fatsecret";
export const FATSECRET_KEYRING_KEYS = Object.freeze({
  consumerSecret: "integrations.fatsecret.consumer_secret",
  accessToken: "integrations.fatsecret.access_token",
  accessTokenSecret: "integrations.fatsecret.access_token_secret",
});

export class FatSecretError extends Error {
  constructor(kind, message, cause) {
    super(message);
    this.name = "FatSecretError";
    this.kind = kind;
    this.cause = cause;
  }
}

function publicError(kind, message, cause) {
  const safe = String(message ?? "request failed")
    .replace(/oauth_[a-z_]+=[^&\s]*/gi, "oauth credential redacted")
    .replace(/(token|secret|key)=?[^&\s]*/gi, "$1 redacted");
  return new FatSecretError(kind, safe, cause);
}

function assertConfig(config) {
  if (
    !config ||
    config.provider !== FATSECRET_PROVIDER ||
    typeof config.clientId !== "string" ||
    !config.clientId ||
    typeof config.baseUrl !== "string"
  ) {
    throw new FatSecretError("config", "FatSecret integration configuration is invalid");
  }
  for (const key of Object.keys(config)) {
    if (/(secret|token|password|credential)/i.test(key))
      throw new FatSecretError("config", "FatSecret secrets must stay in the OS keyring");
  }
}

function parseResponse(response) {
  if (!response || response.status !== 200) {
    const kind =
      response?.status === 401 ? "auth" : response?.status === 429 ? "rate_limit" : "api";
    throw new FatSecretError(
      kind,
      response?.status === 401
        ? "FatSecret authorization failed"
        : response?.status === 429
          ? "FatSecret rate limit reached"
          : "FatSecret API request failed",
    );
  }
  return response.body;
}

function diaryEntries(body) {
  const container = body?.diary_entries ?? body;
  const entries = container?.food_entry ?? container;
  if (Array.isArray(entries)) return entries;
  return entries && typeof entries === "object" ? [entries] : [];
}

export class FatSecretIntegration {
  #syncPromise = null;
  #timer = null;
  #connected = false;

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
    this.config = Object.freeze({
      provider: FATSECRET_PROVIDER,
      baseUrl: config.baseUrl.replace(/\/$/, ""),
      clientId: config.clientId,
    });
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
    };
  }

  async #oauthRequest(url, { token, tokenSecret, params = {} } = {}) {
    const consumerSecret = await this.keyring.get(FATSECRET_KEYRING_KEYS.consumerSecret);
    if (!consumerSecret)
      throw new FatSecretError(
        "keyring",
        "FatSecret consumer secret is unavailable in the OS keyring",
      );
    const signed = signRequest({
      method: "POST",
      url,
      params,
      consumerKey: this.config.clientId,
      consumerSecret,
      token,
      tokenSecret,
      oauthNonce: this.random(),
      oauthTimestamp: this.clock(),
    });
    try {
      const response = await this.http.request({
        method: "POST",
        url,
        headers: {
          Authorization: signed.authorization,
          "Content-Type": "application/x-www-form-urlencoded",
        },
      });
      return parseFormEncoded(parseResponse(response));
    } catch (error) {
      if (error instanceof FatSecretError) throw error;
      throw publicError("transport", error.message, error);
    }
  }

  async connect({ openBrowser, verifier }) {
    if (typeof openBrowser !== "function" || typeof verifier !== "string" || !verifier)
      throw new FatSecretError(
        "oauth",
        "FatSecret OAuth requires a browser opener and callback verifier",
      );
    let request;
    try {
      request = await this.#oauthRequest(`${this.config.baseUrl}/oauth/request_token`, {
        params: { oauth_callback: "http://127.0.0.1/callback" },
      });
      if (!request.oauth_token || !request.oauth_token_secret)
        throw new FatSecretError("oauth", "FatSecret did not return a request token");
      await openBrowser(
        `${this.config.baseUrl}/oauth/authorize?oauth_token=${encodeURIComponent(request.oauth_token)}`,
      );
      const access = await this.#oauthRequest(`${this.config.baseUrl}/oauth/access_token`, {
        token: request.oauth_token,
        tokenSecret: request.oauth_token_secret,
        params: { oauth_verifier: verifier },
      });
      if (!access.oauth_token || !access.oauth_token_secret)
        throw new FatSecretError("oauth", "FatSecret did not return an access token");
      await this.keyring.set(FATSECRET_KEYRING_KEYS.accessToken, access.oauth_token);
      await this.keyring.set(FATSECRET_KEYRING_KEYS.accessTokenSecret, access.oauth_token_secret);
      this.#connected = true;
      return this.status();
    } catch (error) {
      if (error instanceof FatSecretError) throw error;
      throw publicError("oauth", error.message, error);
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
    const token = await this.keyring.get(FATSECRET_KEYRING_KEYS.accessToken);
    const tokenSecret = await this.keyring.get(FATSECRET_KEYRING_KEYS.accessTokenSecret);
    const consumerSecret = await this.keyring.get(FATSECRET_KEYRING_KEYS.consumerSecret);
    if (!token || !tokenSecret || !consumerSecret)
      throw new FatSecretError("auth", "Connect FatSecret before syncing");
    const params = {};
    if (from) params.from = from;
    if (to) params.to = to;
    const url = `${this.config.baseUrl}/diary`;
    const signed = signRequest({
      method: "GET",
      url,
      params,
      consumerKey: this.config.clientId,
      consumerSecret,
      token,
      tokenSecret,
      oauthNonce: this.random(),
      oauthTimestamp: this.clock(),
    });
    let response;
    try {
      response = await this.http.request({
        method: "GET",
        url,
        headers: { Authorization: signed.authorization },
      });
      const body = JSON.parse(parseResponse(response));
      const entries = diaryEntries(body);
      for (const entry of entries) await this.writer.upsertObject(mapDiaryEntry(entry));
      if (typeof this.writer.recordSync === "function") {
        await this.writer.recordSync({
          provider: FATSECRET_PROVIDER,
          from,
          to,
          imported: entries.length,
          completedAt: new Date(this.clock() * 1000).toISOString(),
        });
      }
      this.#connected = true;
      return { imported: entries.length, from, to };
    } catch (error) {
      if (error instanceof FatSecretError) throw error;
      throw publicError(
        response?.status === 401 ? "auth" : response?.status === 429 ? "rate_limit" : "api",
        error.message,
        error,
      );
    }
  }

  async start({ intervalMs = 24 * 60 * 60 * 1000, from, to } = {}) {
    if (this.#timer) return this.status();
    await this.syncNow({ from, to });
    this.#timer = setInterval(() => {
      void this.syncNow({ from, to }).catch(() => {});
    }, intervalMs);
    return this.status();
  }

  stop() {
    if (this.#timer) clearInterval(this.#timer);
    this.#timer = null;
    return this.status();
  }

  async disconnect() {
    this.stop();
    for (const key of [
      FATSECRET_KEYRING_KEYS.accessToken,
      FATSECRET_KEYRING_KEYS.accessTokenSecret,
      FATSECRET_KEYRING_KEYS.consumerSecret,
    ])
      await this.keyring.delete(key);
    this.#connected = false;
    return this.status();
  }
}
