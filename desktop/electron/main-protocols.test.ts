import { afterEach, expect, mock, test } from "../test-support/node-test.mjs";
import fs from "node:fs";
import os from "node:os";
import path from "node:path";

type ProtocolHandler = (request: { url: string }) => Promise<Response>;
const handlers = new Map<string, ProtocolHandler>();

mock.module("electron", () => ({
  protocol: {
    handle: (scheme: string, handler: ProtocolHandler) => {
      handlers.set(scheme, handler);
    },
  },
}));
mock.module("../electron/logging", () => ({
  keplerLog: { info: () => undefined, warn: () => undefined, error: () => undefined },
}));

const { registerMainProtocols } = await import("./main-protocols");
const { localImageUrl, LOCAL_IMAGE_PROTOCOL } =
  await import("../../shared/electron/local-image-protocol");

const roots: string[] = [];
afterEach(() => {
  for (const root of roots.splice(0)) fs.rmSync(root, { recursive: true, force: true });
});

function localImageHandler(): ProtocolHandler {
  registerMainProtocols({
    awaitArkReady: async () => {
      throw new Error("ark not needed for local images");
    },
  });
  const handler = handlers.get(LOCAL_IMAGE_PROTOCOL);
  if (!handler) throw new Error("local image protocol not registered");
  return handler;
}

test("local image protocol serves small images", async () => {
  const root = fs.mkdtempSync(path.join(os.tmpdir(), "kosmos-local-image-"));
  roots.push(root);
  const png = path.join(root, "tiny.png");
  fs.writeFileSync(png, Buffer.from([0x89, 0x50, 0x4e, 0x47]));
  const response = await localImageHandler()({ url: localImageUrl(png) });
  expect(response.status).toBe(200);
});

test("local image protocol rejects oversized image reads", async () => {
  const root = fs.mkdtempSync(path.join(os.tmpdir(), "kosmos-local-image-"));
  roots.push(root);
  const png = path.join(root, "huge.png");
  const oversized = 10 * 1024 * 1024 + 1;
  const file = fs.openSync(png, "w");
  fs.writeSync(file, Buffer.alloc(1024 * 1024), 0, 1024 * 1024, oversized - 1024 * 1024);
  fs.closeSync(file);
  const response = await localImageHandler()({ url: localImageUrl(png) });
  expect(response.status).not.toBe(200);
});
