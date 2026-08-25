import net from "node:net";
import path from "node:path";

export type ServiceRequest = {
  op: "add" | "remove" | "reset" | "status" | "ping";
  domains?: string[];
};

export type FocusServiceResponse = {
  ok: boolean;
  active_domains?: string[];
  error?: string;
  pong?: boolean;
};
export type FocusServiceCliResult = {
  ok: boolean;
  installed?: boolean;
  running?: boolean;
  service_name?: string;
  needs_elevation?: boolean;
  error?: string;
};

type JsonPrimitive = boolean | number | string | null;
type JsonValue = JsonPrimitive | JsonValue[] | JsonRecord;
interface JsonRecord {
  [key: string]: JsonValue;
}

const isJsonRecord = (value: JsonValue): value is JsonRecord =>
  value !== null && typeof value === "object" && !Array.isArray(value);

const isString = (value: JsonValue): value is string => typeof value === "string";

const isCliResult = (value: JsonValue): value is FocusServiceCliResult => {
  if (!isJsonRecord(value) || typeof value.ok !== "boolean") return false;
  return (value.installed === undefined || typeof value.installed === "boolean") &&
    (value.running === undefined || typeof value.running === "boolean") &&
    (value.service_name === undefined || typeof value.service_name === "string") &&
    (value.needs_elevation === undefined || typeof value.needs_elevation === "boolean") &&
    (value.error === undefined || typeof value.error === "string");
};

const isServiceResponse = (value: JsonValue): value is FocusServiceResponse => {
  if (!isJsonRecord(value) || typeof value.ok !== "boolean") return false;
  return (value.active_domains === undefined ||
      (Array.isArray(value.active_domains) && value.active_domains.every(isString))) &&
    (value.error === undefined || typeof value.error === "string") &&
    (value.pong === undefined || typeof value.pong === "boolean");
};

export function parseCliResult(text: string): FocusServiceCliResult {
  const value: JsonValue = JSON.parse(text);
  return isCliResult(value) ? value : { ok: false, error: "invalid cli response shape" };
}
export function parseServiceResponse(text: string): FocusServiceResponse {
  const value: JsonValue = JSON.parse(text);
  return isServiceResponse(value) ? value : { ok: false, error: "invalid service response shape" };
}
export function resolveFocusServicePath(input: {
  packaged: boolean;
  resourcesPath: string;
  appPath: string;
}): string {
  return input.packaged
    ? path.join(input.resourcesPath, "Kosmos System Service.exe")
    : path.resolve(input.appPath, "..", "..", "target", "release", "kepler-focus-svc.exe");
}

export function sendViaPipePath(
  pipePath: string,
  req: ServiceRequest,
): Promise<FocusServiceResponse> {
  return new Promise((resolve) => {
    let resolved = false;
    const finish = (response: FocusServiceResponse) => {
      if (!resolved) {
        resolved = true;
        resolve(response);
      }
    };
    let buffer = "";
    const client = net.connect(pipePath);
    client.on("connect", () => {
      try {
        client.write(JSON.stringify(req) + "\n");
      } catch (error) {
        finish({ ok: false, error: `pipe write: ${error instanceof Error ? error.message : String(error)}` });
        client.destroy();
      }
    });
    client.on("data", (chunk: Buffer) => {
      buffer += chunk.toString("utf8");
      try {
        finish(parseServiceResponse(buffer.trim()));
        client.end();
      } catch {
        // Wait for the complete JSON frame.
      }
    });
    client.on("end", () => {
      if (!resolved) {
        try {
          finish(parseServiceResponse(buffer.trim()));
        } catch (error) {
          finish({ ok: false, error: `pipe parse: ${error instanceof Error ? error.message : String(error)}` });
        }
      }
    });
    client.on("error", (error: NodeJS.ErrnoException) => {
      finish({ ok: false, error: `pipe ${error.code ?? ""}: ${error.message}` });
    });
    setTimeout(() => finish({ ok: false, error: "pipe timeout" }), 3000);
  });
}
