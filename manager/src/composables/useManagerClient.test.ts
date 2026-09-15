import { describe, expect, test } from "../../test-support/node-test.mjs";
import { useManagerClient } from "./useManagerClient";
import type { EngineHealth, EngineInfo, ManagerApi, ManagerResult } from "../manager-api";

const sleep = (ms: number) => new Promise<void>((resolve) => setTimeout(resolve, ms));
const HEALTHY: ManagerResult<EngineHealth> = {
  ok: true,
  data: { ok: true, status: "ready", api_version: "1" },
};
const DOWN: ManagerResult<EngineHealth> = {
  ok: false,
  code: "engine_unavailable",
  message: "Engine недоступен.",
};
const INFO: ManagerResult<EngineInfo> = {
  ok: true,
  data: {
    ok: true,
    api_version: "1",
    legacy_protocol_version: 1,
    pid: 1,
    ws_port: 0,
    correlation_id: "test",
  },
};

function fakeApi(handlers: {
  healthy: () => boolean;
  info?: () => ManagerResult<EngineInfo>;
}): ManagerApi {
  const stub: Pick<ManagerApi, "getHealth" | "getInfo"> = {
    getHealth: () => Promise.resolve(handlers.healthy() ? HEALTHY : DOWN),
    getInfo: () => Promise.resolve(handlers.info?.() ?? INFO),
  };
  // SAFETY: tests stub only the methods they invoke; the rest of ManagerApi is never called.
  return stub as ManagerApi;
}

describe("ManagerClient engine connectivity", () => {
  test("cold start keeps the red banner hidden while connecting", async () => {
    let healthy = false;
    const client = useManagerClient({
      api: fakeApi({ healthy: () => healthy }),
      graceMs: 40,
      probeMs: 10,
    });
    await client.call("getHealth", undefined, "health");
    expect(client.engine.value).toBe("connecting");
    expect(client.error.value).toBe("Engine недоступен.");
    expect(client.banner.value).toBe(null);
    healthy = true;
    await sleep(60);
    expect(client.engine.value).toBe("ready");
    expect(client.error.value).toBe(null);
    expect(client.banner.value).toBe(null);
  });

  test("sustained outage still surfaces the banner after the grace window", async () => {
    const client = useManagerClient({
      api: fakeApi({ healthy: () => false }),
      graceMs: 40,
      probeMs: 10,
    });
    await client.call("getHealth", undefined, "health");
    expect(client.banner.value).toBe(null);
    await sleep(90);
    expect(client.engine.value).toBe("failed");
    expect(client.banner.value).toBe("Engine недоступен.");
  });

  test("non-connectivity errors surface immediately", async () => {
    const client = useManagerClient({
      api: fakeApi({
        healthy: () => true,
        info: () => ({ ok: false, code: "engine", message: "Операция не удалась" }),
      }),
      graceMs: 40,
      probeMs: 10,
    });
    await client.call("getHealth", undefined, "health");
    expect(client.engine.value).toBe("ready");
    await client.call("getInfo", undefined, "info");
    expect(client.banner.value).toBe("Операция не удалась");
    expect(client.engine.value).toBe("ready");
  });

  test("a transient blip inside the grace window never flashes the banner", async () => {
    let healthy = true;
    const client = useManagerClient({
      api: fakeApi({ healthy: () => healthy }),
      graceMs: 50,
      probeMs: 10,
    });
    await client.call("getHealth", undefined, "health");
    expect(client.engine.value).toBe("ready");
    healthy = false;
    await client.call("getHealth", undefined, "health");
    expect(client.banner.value).toBe(null);
    expect(client.engine.value).toBe("connecting");
    healthy = true;
    await sleep(30);
    expect(client.engine.value).toBe("ready");
    await sleep(60);
    expect(client.banner.value).toBe(null);
  });
});
