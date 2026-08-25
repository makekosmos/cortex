import { describe, expect, test } from "bun:test";
import { ArkBroker, type ArkRequest } from "./brokers/ark";
import { createHostBrokers } from "./brokers";
import { HostBrokerError } from "./brokers/types";

const context = {
  launchId: "l",
  packageId: "pkg",
  packageVersion: "1.0.0",
  manifestDigest: "d",
  sessionId: "s",
  origin: "https://pkg.invalid",
  generation: 0,
} as const;
const transport = async (_context: typeof context, request: ArkRequest) => request;

describe("typed host brokers", () => {
  test("forwards authenticated typed requests without exposing authority", async () => {
    const brokers = createHostBrokers(context, {
      ark: transport,
      storage: transport,
      network: transport,
      dialog: transport,
      window: transport,
    });
    await expect(
      brokers.storage.request(context, {
        kind: "read",
        rootCapabilityId: "root",
        relativePath: "a",
        maxBytes: 10,
      }),
    ).resolves.toMatchObject({ kind: "read" });
  });
  test("rejects raw operation params escape hatch", async () => {
    const broker = new ArkBroker(transport);
    expect(() => broker.request(context, { kind: "raw", operation: "read", params: {} })).toThrow(
      "CONTRACT_INVALID",
    );
  });
  test("rejects invalid origin with structured error", () => {
    const brokers = createHostBrokers(context, {
      ark: transport,
      storage: transport,
      network: transport,
      dialog: transport,
      window: transport,
    });
    expect(() =>
      brokers.window.request(
        { ...context, origin: "file:///x" },
        { kind: "perform", action: "close" },
      ),
    ).toThrow(HostBrokerError);
  });
});
