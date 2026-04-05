import { describe, expect, it } from "vitest";

import {
  computeAuthHmac,
  computeMeshId,
  createPeerHello,
  createPeerHelloAck,
  generateNonce,
  verifyAuthHmac,
} from "../peer-protocol";

describe("peer-protocol", () => {
  const meshSecret = "test-mesh-secret-12345";

  it("computeMeshId is deterministic", () => {
    const id1 = computeMeshId(meshSecret);

    const id2 = computeMeshId(meshSecret);

    expect(id1).toBe(id2);

    expect(id1).toHaveLength(16);

    // Different secret produces different id

    expect(computeMeshId("other-secret")).not.toBe(id1);
  });

  it("HMAC verify valid key", () => {
    const nonce = generateNonce();

    const hmac = computeAuthHmac(meshSecret, nonce);

    expect(verifyAuthHmac(meshSecret, nonce, hmac)).toBe(true);
  });

  it("HMAC verify invalid key", () => {
    const nonce = generateNonce();

    const hmac = computeAuthHmac(meshSecret, nonce);

    expect(verifyAuthHmac("wrong-secret", nonce, hmac)).toBe(false);
  });

  it("HMAC verify rejects tampered hmac", () => {
    const nonce = generateNonce();

    const hmac = computeAuthHmac(meshSecret, nonce);

    // Flip a character

    const tampered = hmac.slice(0, -1) + (hmac.endsWith("0") ? "1" : "0");

    expect(verifyAuthHmac(meshSecret, nonce, tampered)).toBe(false);
  });

  it("createPeerHello has valid fields", () => {
    const hello = createPeerHello(
      "device-1",

      "Test Device",

      "electron",

      meshSecret,
    );

    expect(hello.type).toBe("peer_hello");

    expect(hello.protocol_version).toBe(2);

    expect(hello.device_id).toBe("device-1");

    expect(hello.device_name).toBe("Test Device");

    expect(hello.platform).toBe("electron");

    expect(hello.mesh_id).toBe(computeMeshId(meshSecret));

    expect(hello.nonce).toHaveLength(64); // 32 bytes hex

    expect(hello.auth_hmac).toHaveLength(64); // SHA-256 hex

    // HMAC should verify

    expect(verifyAuthHmac(meshSecret, hello.nonce, hello.auth_hmac)).toBe(true);
  });

  it("createPeerHelloAck has valid fields", () => {
    const ack = createPeerHelloAck(
      true,

      "device-2",

      "Server",

      "electron",

      meshSecret,
    );

    expect(ack.type).toBe("peer_hello_ack");

    expect(ack.ok).toBe(true);

    expect(ack.device_id).toBe("device-2");

    expect(ack.nonce).toHaveLength(64);

    expect(verifyAuthHmac(meshSecret, ack.nonce, ack.auth_hmac)).toBe(true);

    expect(ack.error).toBeUndefined();
  });

  it("createPeerHelloAck with error", () => {
    const ack = createPeerHelloAck(
      false,

      "device-2",

      "Server",

      "electron",

      meshSecret,

      "auth failed",
    );

    expect(ack.ok).toBe(false);

    expect(ack.error).toBe("auth failed");
  });

  it("nonce uniqueness", () => {
    const nonces = new Set<string>();

    for (let i = 0; i < 100; i++) {
      nonces.add(generateNonce());
    }

    expect(nonces.size).toBe(100);
  });
});
