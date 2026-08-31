import assert from "node:assert/strict";
import test from "node:test";

import { parseFormEncoded, parseOAuthHeader, signRequest } from "../oauth1.mjs";
import { FATSECRET_KEYRING_KEYS } from "../provider.mjs";
import { makeIntegration } from "./support.mjs";

function accepts(request, token, tokenSecret) {
  const oauth = parseOAuthHeader(request.headers.Authorization);
  const expected = signRequest({
    method: request.method,
    url: request.url,
    consumerKey: "client-key",
    consumerSecret: "consumer-secret",
    token,
    tokenSecret,
    oauthNonce: oauth.oauth_nonce,
    oauthTimestamp: oauth.oauth_timestamp,
    oauthVersion: oauth.oauth_version,
  });
  return oauth.oauth_token === token && oauth.oauth_signature === expected.signature;
}

test("OAuth signing fixture and form parser remain deterministic", () => {
  const signed = signRequest({
    method: "POST",
    url: "http://example.com/request?b5=%3D%253D&a3=a&c%40=&a2=r%20b",
    params: { c2: "", a3: ["2 q"] },
    consumerKey: "9djdj82h48djs9d2",
    consumerSecret: "j49sk3j29djd",
    token: "kkk9d7dh3k39sjv7",
    tokenSecret: "dh893hdasih9",
    oauthNonce: "7d8f3e4a",
    oauthTimestamp: 137131201,
  });
  assert.equal(signed.signature, "r6/TJjbCOr97/+UU0NsvSne7s5g=");
  assert.equal(parseOAuthHeader(signed.authorization).oauth_signature, signed.signature);
  assert.deepEqual(parseFormEncoded("oauth_token=a%2Bb&oauth_verifier=hello+world"), {
    oauth_token: "a+b",
    oauth_verifier: "hello world",
  });
});

test("connect waits for callback and binds access token to its request token", async () => {
  const requests = [];
  const order = [];
  const { integration, values } = makeIntegration({
    http: {
      async request(request) {
        requests.push(request);
        if (requests.length === 1)
          return {
            status: 200,
            body: "oauth_token=request-token&oauth_token_secret=request-secret",
          };
        const url = new URL(request.url);
        assert.equal(url.searchParams.get("oauth_verifier"), "callback-verifier");
        assert.equal(accepts(request, "request-token", "request-secret"), true);
        assert.equal(accepts(request, "other-token", "other-secret"), false);
        return { status: 200, body: "oauth_token=access-token&oauth_token_secret=access-secret" };
      },
    },
  });
  await integration.connect({
    openBrowser: async (url) => {
      order.push("browser");
      assert.match(url, /oauth_token=request-token/);
    },
    waitForCallback: async () => {
      order.push("callback");
      return { oauthToken: "request-token", verifier: "callback-verifier" };
    },
    callbackUrl: "https://app.example.test/oauth/callback",
    verifier: "must-not-be-used",
  });
  assert.deepEqual(order, ["browser", "callback"]);
  assert.equal(requests[0].url, "https://authentication.fatsecret.com/oauth/request_token");
  assert.equal(requests[1].method, "GET");
  assert.equal(new URL(requests[1].url).searchParams.get("oauth_verifier"), "callback-verifier");
  assert.equal(values.get("integrations.fatsecret.access_token"), "access-token");
});

test("connect denies a callback for a different request token or empty verifier", async () => {
  for (const callback of [
    { oauthToken: "other-token", verifier: "callback-verifier" },
    { oauthToken: "request-token", verifier: "" },
  ]) {
    const requests = [];
    const { integration } = makeIntegration({
      http: {
        async request(request) {
          requests.push(request);
          return {
            status: 200,
            body: "oauth_token=request-token&oauth_token_secret=request-secret",
          };
        },
      },
    });
    await assert.rejects(
      integration.connect({
        openBrowser: async () => {},
        waitForCallback: async () => callback,
        callbackUrl: "https://app.example.test/oauth/callback",
      }),
      { kind: "oauth" },
    );
    assert.equal(requests.length, 1);
  }
});

test("disconnect removes credentials written by an in-flight connect", async () => {
  const values = new Map([[FATSECRET_KEYRING_KEYS.consumerSecret, "consumer-secret"]]);
  let releaseSet;
  let setStarted;
  const blocked = new Promise((resolve) => {
    releaseSet = resolve;
  });
  const started = new Promise((resolve) => {
    setStarted = resolve;
  });
  const keyring = {
    async get(key) {
      return values.get(key);
    },
    async set(key, value) {
      setStarted();
      await blocked;
      values.set(key, value);
    },
    async delete(key) {
      values.delete(key);
    },
  };
  let requestCount = 0;
  const { integration } = makeIntegration({
    keyring,
    http: {
      async request() {
        requestCount += 1;
        return requestCount === 1
          ? {
              status: 200,
              body: "oauth_token=request-token&oauth_token_secret=request-secret",
            }
          : { status: 200, body: "oauth_token=access-token&oauth_token_secret=access-secret" };
      },
    },
  });
  const connecting = integration.connect({
    openBrowser: async () => {},
    waitForCallback: async () => ({
      oauthToken: "request-token",
      verifier: "callback-verifier",
    }),
    callbackUrl: "https://app.example.test/oauth/callback",
  });
  await started;
  await integration.disconnect();
  releaseSet();
  await assert.rejects(connecting, { kind: "oauth" });
  assert.equal(values.has(FATSECRET_KEYRING_KEYS.accessToken), false);
  assert.equal(values.has(FATSECRET_KEYRING_KEYS.accessTokenSecret), false);
  assert.equal(integration.status().connected, false);
});
