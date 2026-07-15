import { lookup } from "node:dns/promises";
import { createServer } from "node:http";
import { connect, type Socket } from "node:net";
import type { Duplex } from "node:stream";
import { isPublicNetworkAddress } from "./public-network-address";

type ResolveAddresses = (hostname: string) => Promise<Array<{ address: string; family: number }>>;
type ConnectToAddress = (address: string, port: number, family: number) => Socket;

export interface BookMetadataConnectProxy {
  config: {
    proxyRules: string;
    proxyBypassRules: "<-loopback>";
  };
  close(): Promise<void>;
}

function parseConnectAuthority(authority: string): { hostname: string; port: number } {
  const url = new URL(`https://${authority}`);
  const hostname = url.hostname.replace(/^\[|\]$/g, "");
  const port = Number(url.port || 443);
  if (
    !hostname ||
    url.username ||
    url.password ||
    url.pathname !== "/" ||
    url.search ||
    url.hash ||
    !Number.isInteger(port) ||
    port < 1 ||
    port > 65_535
  ) {
    throw new Error("Некорректный CONNECT target");
  }
  return { hostname, port };
}

function rejectConnect(socket: Duplex): void {
  if (!socket.destroyed) {
    socket.end("HTTP/1.1 502 Bad Gateway\r\nConnection: close\r\n\r\n");
  }
}

export async function createBookMetadataConnectProxy(
  dependencies: {
    resolveAddresses?: ResolveAddresses;
    connectToAddress?: ConnectToAddress;
  } = {},
): Promise<BookMetadataConnectProxy> {
  const resolveAddresses =
    dependencies.resolveAddresses ??
    ((hostname: string) => lookup(hostname, { all: true, verbatim: true }));
  const connectToAddress =
    dependencies.connectToAddress ??
    ((address: string, port: number, family: number) => connect({ host: address, port, family }));
  const sockets = new Set<Duplex>();
  let closed = false;

  const server = createServer((_request, response) => {
    response.writeHead(403, { Connection: "close", "Content-Type": "text/plain" });
    response.end("HTTPS CONNECT required");
  });
  server.on("connection", (socket) => {
    sockets.add(socket);
    socket.once("close", () => sockets.delete(socket));
  });
  server.on("clientError", (_error, socket) => socket.destroy());
  server.on("connect", (request, client, head) => {
    void (async () => {
      let target: { hostname: string; port: number };
      try {
        target = parseConnectAuthority(request.url ?? "");
        const addresses = await resolveAddresses(target.hostname);
        if (
          closed ||
          client.destroyed ||
          !addresses.length ||
          addresses.some(({ address }) => !isPublicNetworkAddress(address))
        ) {
          rejectConnect(client);
          return;
        }

        const selected = addresses[0]!;
        // Security boundary: never resolve the hostname again. See postmortems.md § 2026-07-14.
        const upstream = connectToAddress(selected.address, target.port, selected.family);
        sockets.add(upstream);
        upstream.once("close", () => sockets.delete(upstream));
        let connected = false;
        upstream.once("connect", () => {
          if (closed || client.destroyed) {
            upstream.destroy();
            return;
          }
          connected = true;
          client.write("HTTP/1.1 200 Connection Established\r\n\r\n");
          if (head.length) upstream.write(head);
          client.pipe(upstream);
          upstream.pipe(client);
        });
        upstream.once("error", () => {
          if (!connected) rejectConnect(client);
          else client.destroy();
        });
        client.once("error", () => upstream.destroy());
        client.once("close", () => upstream.destroy());
      } catch {
        rejectConnect(client);
      }
    })();
  });

  await new Promise<void>((resolve, reject) => {
    const fail = (error: Error) => reject(error);
    server.once("error", fail);
    server.listen(0, "127.0.0.1", () => {
      server.removeListener("error", fail);
      resolve();
    });
  });
  const address = server.address();
  if (!address || typeof address === "string") {
    server.close();
    throw new Error("Не удалось запустить локальный CONNECT proxy");
  }
  const rule = `127.0.0.1:${address.port}`;

  return {
    config: {
      proxyRules: `http=${rule};https=${rule}`,
      proxyBypassRules: "<-loopback>",
    },
    async close() {
      if (closed) return;
      closed = true;
      for (const socket of sockets) socket.destroy();
      await new Promise<void>((resolve) => server.close(() => resolve()));
    },
  };
}
