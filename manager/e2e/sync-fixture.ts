import fs from "node:fs";
import { createServer, type Server } from "node:http";
import path from "node:path";

export const fixtureTicket = "fixture-ticket-123";
export const fixtureAuthToken = "a".repeat(64);

export function delay(ms: number) {
  return new Promise((resolve) => setTimeout(resolve, ms));
}

export type SyncFixture = {
  unavailable: boolean;
  ticketFailure: boolean;
  connectFailure: boolean;
  disconnectFailure: boolean;
  concurrentSnapshots: number;
  maxConcurrentSnapshots: number;
  connectCalls: number;
  connected: boolean;
  disconnected: boolean;
  close(): Promise<void>;
};

export async function startSyncFixture(dataDir: string): Promise<SyncFixture> {
  const fixture = {
    unavailable: false,
    ticketFailure: false,
    connectFailure: false,
    disconnectFailure: false,
    concurrentSnapshots: 0,
    maxConcurrentSnapshots: 0,
    connectCalls: 0,
    connected: false,
    disconnected: false,
  };
  const server: Server = createServer(async (request, response) => {
    type SyncBody = {
      [key: string]: string | number | boolean | null | undefined;
    };
    const send = (body: SyncBody) => {
      response.writeHead(200, { "Content-Type": "application/json" });
      response.end(JSON.stringify(body));
    };
    if (request.url === "/v1/health")
      return send({ ok: true, status: "ready", api_version: "1.0.0" });
    if (request.url !== "/v1/rpc" || request.method !== "POST") return send({ ok: false });
    const chunks: Buffer[] = [];
    for await (const chunk of request) chunks.push(Buffer.from(chunk));
    // SAFETY: the fixture request body is a JSON object from the sync HTTP client.
    const body = JSON.parse(Buffer.concat(chunks).toString("utf8")) as SyncBody;
    if (body.operation === "get_sync_snapshot") {
      fixture.concurrentSnapshots += 1;
      fixture.maxConcurrentSnapshots = Math.max(
        fixture.maxConcurrentSnapshots,
        fixture.concurrentSnapshots,
      );
      await delay(25);
      fixture.concurrentSnapshots -= 1;
      if (fixture.unavailable) return send({ ok: false, error: "fixture unavailable" });
      return send({
        ok: true,
        data: {
          running: true,
          transport: "iroh",
          pairing_available: true,
          own_pairing_code_available: true,
          local_device: {
            device_id: "fixture-manager",
            device_name: "Тестовый Manager",
          },
          peers: [
            !fixture.disconnected && {
              device_id: "fixture-online",
              device_name: "Телефон",
              status: "online",
              last_seen: "2026-08-01T10:20:30Z",
            },
            {
              device_id: "fixture-offline",
              device_name: "Планшет",
              status: "offline",
              last_seen: "2026-08-01T09:20:30Z",
            },
            fixture.connected && {
              device_id: "fixture-connected",
              device_name: "Подключённое устройство",
              status: "online",
              last_seen: "2026-08-01T10:21:30Z",
            },
          ].filter(Boolean),
        },
      });
    }
    if (body.operation === "get_own_iroh_ticket")
      return fixture.ticketFailure
        ? send({ ok: false, error: "secret-ticket-failure" })
        : send({ ok: true, data: ` ${fixtureTicket} ` });
    if (body.operation === "connect_with_pairing_code") {
      fixture.connectCalls += 1;
      if (fixture.connectFailure) return send({ ok: false, error: "secret-connect-failure" });
      fixture.connected = body.pairing_code === "valid-code-123";
      return send({ ok: true, data: true });
    }
    if (body.operation === "disconnect_peer") {
      if (fixture.disconnectFailure) return send({ ok: false, error: "secret-disconnect-failure" });
      fixture.disconnected = body.device_id === "fixture-online";
      return send({ ok: true, data: true });
    }
    return send({ ok: true, data: {} });
  });
  await new Promise<void>((resolve) => server.listen(0, "127.0.0.1", resolve));
  const address = server.address();
  if (!address) throw new Error("Sync fixture did not bind a port");
  fs.mkdirSync(dataDir, { recursive: true });
  fs.writeFileSync(
    path.join(dataDir, "engine.lock.json"),
    JSON.stringify({
      format_version: 1,
      api_version: { major: 1, minor: 0, patch: 0 },
      pid: process.pid,
      http_port: address.port,
      ws_port: 4318,
      auth_token: fixtureAuthToken,
      started_at: "2026-08-01T00:00:00Z",
      correlation_id: "11111111-1111-4111-8111-111111111111",
    }),
  );
  return Object.assign(fixture, {
    close: () =>
      new Promise<void>((resolve, reject) =>
        server.close((error) => (error ? reject(error) : resolve())),
      ),
  });
}
