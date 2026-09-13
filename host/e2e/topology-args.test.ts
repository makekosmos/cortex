import { readFileSync } from "node:fs";
import { describe, expect, test } from "../test-support/node-test.mjs";

const topology = readFileSync(new URL("./topology.spec.ts", import.meta.url), "utf8");
const fixtures = readFileSync(new URL("./fixtures/signed-apps.ts", import.meta.url), "utf8");

describe("isolated Electron topology launches", () => {
  test("passes user-data-dir before each app entrypoint", () => {
    expect(topology).toContain("--user-data-dir=${path.join(userData, randomUUID())}");
    expect(topology).toContain("old.managerMain");
    expect(topology).toContain("old.hostMain");
    expect(topology).toContain("installed.hostMain");
    expect(topology).toMatch(/hostProcess\s*=\s*spawn\(\s*installed\.electron/);
    expect(topology).toMatch(/stdio:\s*\[\s*"ignore",\s*"pipe",\s*"pipe"\s*\]/);
    expect(topology).toContain('stderr.includes("[host-lifecycle] window closed")');
    expect(topology).toContain("processCommandLine(hostPid).includes(installed.hostMain)");
    expect(fixtures).toContain("window.kosmosApp.window.close()");
  });
});
