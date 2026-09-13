import { describe, expect, test } from "./test-support/node-test.mjs";
import { readFileSync } from "node:fs";
import {
  parseCliResult,
  parseServiceResponse,
  resolveFocusServicePath,
} from "./focus-service-protocol";
import { sendViaPipePath } from "./focus-service-protocol";

describe("Focus service client", () => {
  test("accepts only bounded protocol shapes", () => {
    expect(parseCliResult('{"ok":true,"installed":true,"running":false}')).toEqual({
      ok: true,
      installed: true,
      running: false,
    });
    expect(parseCliResult('{"ok":"yes"}').ok).toBe(false);
    expect(parseServiceResponse('{"ok":true,"pong":true}')).toEqual({
      ok: true,
      pong: true,
    });
    expect(parseServiceResponse('{"ok":true,"active_domains":[1]}').ok).toBe(false);
  });
  test("keeps child launches hidden and timeout finite", () => {
    const source = readFileSync(new URL("./focus-service-client.ts", import.meta.url), "utf8");
    const protocol = readFileSync(new URL("./focus-service-protocol.ts", import.meta.url), "utf8");
    expect(source).toContain("windowsHide: true");
    expect(protocol).toContain(
      'setTimeout(() => finish({ ok: false, error: "pipe timeout" }), 3000)',
    );
    expect(source).not.toContain('stdio: "inherit"');
    expect(
      resolveFocusServicePath({
        packaged: false,
        appPath: "C:\\repo\\platform\\manager",
        resourcesPath: "C:\\resources",
      }),
    ).toContain("target\\release\\kepler-focus-svc.exe");
    expect(
      resolveFocusServicePath({
        packaged: true,
        appPath: "C:\\repo",
        resourcesPath: "C:\\resources",
      }),
    ).toBe("C:\\resources\\Kosmos System Service.exe");
  });
  test("returns a bounded error for an unavailable pipe", async () => {
    const started = performance.now();
    const response = await sendViaPipePath("\\\\.\\pipe\\kosmos-missing-focus-test", {
      op: "ping",
    });
    expect(response.ok).toBe(false);
    expect(performance.now() - started).toBeLessThan(3_500);
  });
});
