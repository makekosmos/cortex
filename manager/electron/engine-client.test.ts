import { beforeEach, expect, mock, test } from "bun:test";

let nextState: unknown = {
  kind: "incompatible",
  error: { kind: "incompatible", message: "Версия Engine несовместима. Обновите Kosmos." },
  engineMajor: 2,
  clientMajor: 1,
};

mock.module("electron", () => ({
  app: { getPath: () => "C:\\Kosmos-test", getVersion: () => "9.8.7" },
}));
mock.module("@kosmos/ark", () => ({ ensureEngineRunning: async () => nextState }));

const { connectEngine } = await import("./engine-client");

beforeEach(() => {
  nextState = {
    kind: "incompatible",
    error: { kind: "incompatible", message: "Версия Engine несовместима. Обновите Kosmos." },
    engineMajor: 2,
    clientMajor: 1,
  };
});

test("Manager maps strict Engine incompatible state to safe Russian result", async () => {
  await expect(connectEngine()).resolves.toEqual({
    ok: false,
    code: "incompatible_api",
    message: "Версия Engine несовместима. Обновите Kosmos.",
  });
});
