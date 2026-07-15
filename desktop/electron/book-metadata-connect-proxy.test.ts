import { afterEach, describe, expect, test } from "bun:test";
import { createServer as createTcpServer, connect as connectTcp, type Server } from "node:net";
import { createBookMetadataConnectProxy } from "./book-metadata-connect-proxy";

const servers: Server[] = [];

afterEach(async () => {
  await Promise.all(
    servers
      .splice(0)
      .map((server) => new Promise<void>((resolve) => server.close(() => resolve()))),
  );
});

function listen(server: Server): Promise<number> {
  servers.push(server);
  return new Promise((resolve, reject) => {
    server.once("error", reject);
    server.listen(0, "127.0.0.1", () => {
      const address = server.address();
      if (!address || typeof address === "string") reject(new Error("listen failed"));
      else resolve(address.port);
    });
  });
}

function proxyPort(proxyRules: string): number {
  const match = proxyRules.match(/http=127\.0\.0\.1:(\d+)/);
  if (!match) throw new Error("proxy port missing");
  return Number(match[1]);
}

describe("book metadata CONNECT proxy", () => {
  test("pins the validated IP into the only upstream connection", async () => {
    // Regression: 2026-07-14. Validation and connect must not resolve the hostname separately.
    const targetPort = await listen(createTcpServer((socket) => socket.pipe(socket)));
    const resolvedHosts: string[] = [];
    const connectedTargets: Array<{ address: string; port: number; family: number }> = [];
    const proxy = await createBookMetadataConnectProxy({
      resolveAddresses: async (hostname) => {
        resolvedHosts.push(hostname);
        return [{ address: "93.184.216.34", family: 4 }];
      },
      connectToAddress: (address, port, family) => {
        connectedTargets.push({ address, port, family });
        return connectTcp({ host: "127.0.0.1", port: targetPort });
      },
    });

    try {
      expect(proxy.config.proxyBypassRules).toBe("<-loopback>");
      expect(proxy.config.proxyRules).toContain("http=127.0.0.1:");
      expect(proxy.config.proxyRules).toContain("https=127.0.0.1:");

      const client = connectTcp({ host: "127.0.0.1", port: proxyPort(proxy.config.proxyRules) });
      await new Promise<void>((resolve, reject) => {
        client.once("connect", resolve);
        client.once("error", reject);
      });
      client.write("CONNECT books.example:443 HTTP/1.1\r\nHost: books.example:443\r\n\r\n");
      const response = await new Promise<string>((resolve, reject) => {
        client.once("data", (data) => resolve(data.toString("utf8")));
        client.once("error", reject);
      });
      expect(response).toContain("200 Connection Established");
      client.write("probe");
      const echoed = await new Promise<string>((resolve, reject) => {
        client.once("data", (data) => resolve(data.toString("utf8")));
        client.once("error", reject);
      });
      expect(echoed).toBe("probe");
      client.destroy();

      expect(resolvedHosts).toEqual(["books.example"]);
      expect(connectedTargets).toEqual([{ address: "93.184.216.34", port: 443, family: 4 }]);
    } finally {
      await proxy.close();
    }
  });

  test("denies ordinary HTTP proxy requests without resolving a target", async () => {
    let resolutions = 0;
    const proxy = await createBookMetadataConnectProxy({
      resolveAddresses: async () => {
        resolutions += 1;
        return [{ address: "93.184.216.34", family: 4 }];
      },
    });
    try {
      const response = await fetch("http://127.0.0.1:" + proxyPort(proxy.config.proxyRules), {
        headers: { Host: "books.example" },
      });
      expect(response.status).toBe(403);
      expect(resolutions).toBe(0);
    } finally {
      await proxy.close();
    }
  });

  test("rejects private CONNECT targets before opening an upstream socket", async () => {
    let connects = 0;
    const proxy = await createBookMetadataConnectProxy({
      resolveAddresses: async () => [{ address: "127.0.0.1", family: 4 }],
      connectToAddress: () => {
        connects += 1;
        return connectTcp({ host: "127.0.0.1", port: 1 });
      },
    });
    try {
      const client = connectTcp({
        host: "127.0.0.1",
        port: proxyPort(proxy.config.proxyRules),
      });
      await new Promise<void>((resolve, reject) => {
        client.once("connect", resolve);
        client.once("error", reject);
      });
      client.write("CONNECT internal.example:443 HTTP/1.1\r\nHost: internal.example:443\r\n\r\n");
      const response = await new Promise<string>((resolve, reject) => {
        client.once("data", (data) => resolve(data.toString("utf8")));
        client.once("error", reject);
      });
      expect(response).toContain("502 Bad Gateway");
      expect(connects).toBe(0);
      client.destroy();
    } finally {
      await proxy.close();
    }
  });
});
