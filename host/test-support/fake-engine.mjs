/* oxlint-disable anti-slop/no-runtime-typeof -- the test double parses untyped wire JSON at the boundary. */
// A real HTTP server standing in for the Engine's `/v1/user-data` boundary.
// It mirrors runtime/src/engine_api/handlers/user_data.rs — bearer auth,
// client-class gate, root pinning, `extension-data/<app>/<key>` layout — so
// tests exercise the EngineClient wire contract against genuine HTTP traffic.
import { randomUUID } from "node:crypto";
import { existsSync, mkdirSync, readFileSync, rmSync, statSync, writeFileSync } from "node:fs";
import { createServer } from "node:http";
import path from "node:path";

const POST_LIMIT = 1_048_576;
export const FAKE_ENGINE_MAX_BYTES = 25 * 1024 * 1024;
const COMPONENT = /^[0-9A-Za-z_][0-9A-Za-z_.-]{0,255}$/;

function keyParts(key) {
  if (typeof key !== "string" || key.length === 0 || key.length > 512) return null;
  const parts = key.replaceAll("\\", "/").split("/");
  return parts.every((part) => COMPONENT.test(part)) ? parts : null;
}

export async function startFakeEngine(token) {
  const roots = new Map();
  const byPath = new Map();
  const requests = [];
  const engine = {
    port: 0,
    expectedToken: token,
    requests,
    hijack: null,
    restart() {
      roots.clear();
      byPath.clear();
    },
    close: () =>
      new Promise((resolve) => {
        server.close(() => resolve());
        server.closeAllConnections();
      }),
  };

  const send = (entry, res, status, payload) => {
    entry.status = status;
    res.writeHead(status, { "content-type": "application/json" });
    res.end(typeof payload === "string" ? payload : JSON.stringify(payload));
  };
  const fail = (entry, res, status, error) => send(entry, res, status, { ok: false, error });
  const ok = (entry, res, data) => send(entry, res, 200, { ok: true, data });

  const fileFor = (rootId, appId, key) => {
    const root = roots.get(rootId);
    if (!root) return { error: "unknown-root", status: 404 };
    if (typeof appId !== "string" || !COMPONENT.test(appId))
      return { error: "invalid-request", status: 400 };
    const parts = keyParts(key);
    if (!parts) return { error: "invalid-key", status: 400 };
    return { file: path.join(root, "extension-data", appId, ...parts) };
  };

  const server = createServer((req, res) => {
    const entry = {
      method: req.method ?? "",
      operation: req.method === "PUT" ? "write" : "",
      headers: req.headers,
      bodyBytes: 0,
      status: 0,
    };
    requests.push(entry);
    const chunks = [];
    const cap = req.method === "PUT" ? FAKE_ENGINE_MAX_BYTES : POST_LIMIT;
    let overflow = false;
    req.on("data", (chunk) => {
      entry.bodyBytes += chunk.length;
      if (entry.bodyBytes > cap) overflow = true;
      else chunks.push(chunk);
    });
    req.on("end", () => {
      // Name the operation for the request log even when the request is
      // rejected before the body is consumed.
      if (entry.method === "POST") {
        try {
          const operation = JSON.parse(Buffer.concat(chunks).toString("utf8"))?.operation;
          if (typeof operation === "string") entry.operation = operation;
        } catch {}
      }
      const pathname = new URL(req.url ?? "/", "http://localhost").pathname;
      if (pathname !== "/v1/user-data" || !["POST", "PUT"].includes(entry.method))
        return fail(entry, res, 404, "not found");
      if (req.headers.authorization !== `Bearer ${engine.expectedToken}`)
        return fail(entry, res, 401, "invalid bearer token");
      const pid = Number(req.headers["x-kosmos-client-pid"]);
      if (!Number.isInteger(pid) || pid <= 0) return fail(entry, res, 403, "invalid client PID");
      if (!/^\d+\.\d+\.\d+$/.test(String(req.headers["x-kosmos-api-version"])))
        return fail(entry, res, 426, "missing or incompatible API version");
      if (req.headers["x-kosmos-client-class"] !== "desktop-host")
        return fail(entry, res, 403, "forbidden");
      if (overflow)
        return fail(
          entry,
          res,
          413,
          entry.method === "PUT" ? "too-large" : "request body exceeds 1 MiB",
        );

      if (entry.method === "PUT") {
        const hijacked = engine.hijack?.("write");
        if (hijacked) return send(entry, res, hijacked.status, hijacked.payload);
        const rootId = req.headers["x-kosmos-user-data-root"];
        const appId = req.headers["x-kosmos-user-data-app"];
        const key = req.headers["x-kosmos-user-data-key"];
        if (!rootId || !appId || !key)
          return fail(entry, res, 400, "missing x-kosmos-user-data-* header");
        const target = fileFor(rootId, appId, key);
        if (target.error) return fail(entry, res, target.status, target.error);
        try {
          mkdirSync(path.dirname(target.file), { recursive: true });
          writeFileSync(target.file, Buffer.concat(chunks));
          return ok(entry, res, { size_bytes: entry.bodyBytes });
        } catch {
          return fail(entry, res, 500, "io-error");
        }
      }

      let body;
      try {
        body = JSON.parse(Buffer.concat(chunks).toString("utf8"));
      } catch {
        return fail(entry, res, 400, "malformed user data request");
      }
      entry.operation = typeof body.operation === "string" ? body.operation : "?";
      const hijacked = engine.hijack?.(entry.operation);
      if (hijacked) return send(entry, res, hijacked.status, hijacked.payload);
      const field = (name) =>
        typeof body[name] === "string" && body[name].length > 0 ? body[name] : null;

      if (entry.operation === "open_root") {
        const root = field("root");
        if (!root) return fail(entry, res, 400, "missing root");
        if (!path.isAbsolute(root)) return fail(entry, res, 400, "invalid-request");
        const existing = byPath.get(root);
        if (existing) return ok(entry, res, { root_id: existing });
        if (!existsSync(root) || !statSync(root).isDirectory())
          return fail(entry, res, 404, "not-found");
        const rootId = randomUUID();
        roots.set(rootId, root);
        byPath.set(root, rootId);
        return ok(entry, res, { root_id: rootId });
      }
      if (entry.operation === "close_root") {
        const rootId = field("root_id");
        if (!rootId) return fail(entry, res, 400, "missing root_id");
        const root = roots.get(rootId);
        if (!root) return fail(entry, res, 404, "unknown-root");
        roots.delete(rootId);
        byPath.delete(root);
        return ok(entry, res, { closed: true });
      }
      if (["read", "stat", "delete"].includes(entry.operation)) {
        const rootId = field("root_id");
        const appId = field("app_id");
        const key = field("key");
        if (!rootId) return fail(entry, res, 400, "missing root_id");
        if (!appId) return fail(entry, res, 400, "missing app_id");
        if (!key) return fail(entry, res, 400, "missing key");
        const target = fileFor(rootId, appId, key);
        if (target.error) return fail(entry, res, target.status, target.error);
        try {
          if (entry.operation === "read")
            return ok(entry, res, { bytes: readFileSync(target.file).toString("base64") });
          const stats = statSync(target.file);
          if (!stats.isFile()) return fail(entry, res, 500, "io-error");
          if (entry.operation === "stat") return ok(entry, res, { size_bytes: stats.size });
          rmSync(target.file);
          return ok(entry, res, { deleted: true });
        } catch (error) {
          return fail(
            entry,
            res,
            error?.code === "ENOENT" ? 404 : 500,
            error?.code === "ENOENT" ? "not-found" : "io-error",
          );
        }
      }
      return fail(entry, res, 400, "invalid-request");
    });
  });
  await new Promise((resolve, reject) => {
    server.once("error", reject);
    server.listen(0, "127.0.0.1", resolve);
  });
  engine.port = server.address().port;
  return engine;
}
