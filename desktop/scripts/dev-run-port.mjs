import {
  closeSync,
  existsSync,
  lstatSync,
  mkdirSync,
  openSync,
  realpathSync,
  unlinkSync,
  writeFileSync,
} from "node:fs";
import { randomUUID } from "node:crypto";
import net from "node:net";
import path from "node:path";

const safePort = (port) => Number.isSafeInteger(port) && port > 0 && port < 65536;
const portAvailable = (port) =>
  new Promise((resolve) => {
    const server = net.createServer();
    server.once("error", () => resolve(false));
    server.listen({ host: "127.0.0.1", port, exclusive: true }, () =>
      server.close(() => resolve(true)),
    );
  });

function assertLeasePath(file, root) {
  const boundary = path.resolve(root);
  const leases = path.join(boundary, "leases");
  const fileName = path.basename(file);
  if (!/^\d+\.lease$/.test(fileName) || !safePort(Number.parseInt(fileName, 10)))
    throw new Error("port lease is outside test root");
  if (path.dirname(path.resolve(file)) !== leases)
    throw new Error("port lease is outside test root");
  let current = leases;
  while (current === boundary || current.startsWith(`${boundary}${path.sep}`)) {
    if (existsSync(current)) {
      if (
        lstatSync(current).isSymbolicLink() ||
        path.resolve(realpathSync.native(current)) !== current
      )
        throw new Error("port lease is outside test root");
    }
    if (current === boundary) return;
    current = path.dirname(current);
  }
  throw new Error("port lease is outside test root");
}

export async function acquirePortLease(root, candidatePorts = []) {
  const leaseRoot = path.join(root, "leases");
  mkdirSync(leaseRoot, { recursive: true });
  for (let attempt = 0; attempt < 20; attempt += 1) {
    const port =
      candidatePorts[attempt] ?? 10000 + (Number.parseInt(randomUUID().slice(-4), 16) % 50000);
    if (!safePort(port) || !(await portAvailable(port))) continue;
    const file = path.join(leaseRoot, `${port}.lease`);
    try {
      const fd = openSync(file, "wx");
      writeFileSync(fd, `${process.pid}\n`, "utf8");
      closeSync(fd);
      if (!(await portAvailable(port))) {
        unlinkSync(file, { force: true });
        continue;
      }
      return { port, file, release: () => releasePortLease(file, root) };
    } catch {
      // An existing lease is owned by another run; choose another port.
    }
  }
  throw new Error("unable to acquire a dev port lease");
}

export function releasePortLease(file, root) {
  if (!file) return;
  assertLeasePath(file, root);
  unlinkSync(file, { force: true });
}
