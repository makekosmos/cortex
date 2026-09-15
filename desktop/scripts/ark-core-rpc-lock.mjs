import { randomUUID } from "node:crypto";
import {
  closeSync,
  mkdirSync,
  openSync,
  readFileSync,
  readdirSync,
  renameSync,
  rmSync,
  statSync,
  writeFileSync,
} from "node:fs";
import path from "node:path";

const LOCK_WAIT_MS = 10 * 60 * 1000;
const STALE_LOCK_MS = 5 * 60 * 1000;
const waitBuffer = new Int32Array(new SharedArrayBuffer(4));

function isProcessAlive(pid) {
  if (!Number.isInteger(pid) || pid <= 0) return false;
  try {
    process.kill(pid, 0);
    return true;
  } catch (error) {
    return error?.code === "EPERM";
  }
}

function readTicket(ticketPath) {
  try {
    const ticket = JSON.parse(readFileSync(ticketPath, "utf8"));
    if (!Number.isInteger(Number(ticket.pid)) || ticket.token?.constructor !== String) return null;
    return { pid: Number(ticket.pid), token: ticket.token };
  } catch {
    return null;
  }
}

function ticketPaths(lockPath) {
  const prefix = `${path.basename(lockPath)}.ticket-`;
  return readdirSync(path.dirname(lockPath))
    .filter((name) => name.startsWith(prefix))
    .map((name) => path.join(path.dirname(lockPath), name))
    .sort();
}

function ownerPaths(lockPath) {
  const prefix = `${path.basename(lockPath)}.owner-`;
  return readdirSync(path.dirname(lockPath))
    .filter((name) => name.startsWith(prefix))
    .map((name) => path.join(path.dirname(lockPath), name))
    .sort();
}

function isStale(ticketPath, staleMs) {
  try {
    const ticket = readTicket(ticketPath);
    const age = Date.now() - statSync(ticketPath).mtimeMs;
    return ticket ? !isProcessAlive(ticket.pid) : age > staleMs;
  } catch {
    return false;
  }
}

function activeTickets(lockPath, staleMs) {
  return ticketPaths(lockPath).filter((ticketPath) => {
    if (!isStale(ticketPath, staleMs)) return true;
    rmSync(ticketPath, { force: true });
    return false;
  });
}

function activeOwners(lockPath, staleMs) {
  return ownerPaths(lockPath).filter((ownerPath) => {
    if (!isStale(ownerPath, staleMs)) return true;
    rmSync(ownerPath, { force: true });
    return false;
  });
}

export function acquireCacheLock(
  lockPath,
  { waitMs = LOCK_WAIT_MS, staleMs = STALE_LOCK_MS } = {},
) {
  const directory = path.dirname(lockPath);
  mkdirSync(directory, { recursive: true });
  const token = randomUUID();
  const ticketPath = path.join(directory, `${path.basename(lockPath)}.ticket-${token}`);
  try {
    const fd = openSync(ticketPath, "wx");
    try {
      writeFileSync(fd, JSON.stringify({ pid: process.pid, token, startedAt: Date.now() }));
    } finally {
      closeSync(fd);
    }
  } catch (error) {
    rmSync(ticketPath, { force: true });
    throw error;
  }
  const deadline = Date.now() + waitMs;
  const ownerPath = path.join(directory, `${path.basename(lockPath)}.owner-${token}`);
  try {
    while (true) {
      if (
        activeOwners(lockPath, staleMs).length === 0 &&
        activeTickets(lockPath, staleMs)[0] === ticketPath
      ) {
        try {
          renameSync(ticketPath, ownerPath);
          return () => rmSync(ownerPath, { force: true });
        } catch {}
      }
      if (Date.now() >= deadline) throw new Error(`timed out waiting for cache lock: ${lockPath}`);
      Atomics.wait(waitBuffer, 0, 0, 50);
    }
  } catch (error) {
    rmSync(ticketPath, { force: true });
    throw error;
  }
}
