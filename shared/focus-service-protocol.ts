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
const isRecord = (value: unknown): value is Record<string, unknown> =>
  Boolean(value) && typeof value === "object" && !Array.isArray(value);
export function parseCliResult(text: string): FocusServiceCliResult {
  const value: unknown = JSON.parse(text);
  if (!isRecord(value) || typeof value.ok !== "boolean")
    return { ok: false, error: "invalid cli response shape" };
  if (value.installed !== undefined && typeof value.installed !== "boolean")
    return { ok: false, error: "invalid cli response shape" };
  if (value.running !== undefined && typeof value.running !== "boolean")
    return { ok: false, error: "invalid cli response shape" };
  if (value.service_name !== undefined && typeof value.service_name !== "string")
    return { ok: false, error: "invalid cli response shape" };
  if (value.needs_elevation !== undefined && typeof value.needs_elevation !== "boolean")
    return { ok: false, error: "invalid cli response shape" };
  if (value.error !== undefined && typeof value.error !== "string")
    return { ok: false, error: "invalid cli response shape" };
  return value as FocusServiceCliResult;
}
export function parseServiceResponse(text: string): FocusServiceResponse {
  const value: unknown = JSON.parse(text);
  if (!isRecord(value) || typeof value.ok !== "boolean")
    return { ok: false, error: "invalid service response shape" };
  if (
    value.active_domains !== undefined &&
    (!Array.isArray(value.active_domains) ||
      value.active_domains.some((domain) => typeof domain !== "string"))
  )
    return { ok: false, error: "invalid service response shape" };
  if (value.error !== undefined && typeof value.error !== "string")
    return { ok: false, error: "invalid service response shape" };
  if (value.pong !== undefined && typeof value.pong !== "boolean")
    return { ok: false, error: "invalid service response shape" };
  return value as FocusServiceResponse;
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
        finish({ ok: false, error: `pipe write: ${(error as Error).message}` });
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
          finish({ ok: false, error: `pipe parse: ${(error as Error).message}` });
        }
      }
    });
    client.on("error", (error: NodeJS.ErrnoException) => {
      finish({ ok: false, error: `pipe ${error.code ?? ""}: ${error.message}` });
    });
    setTimeout(() => finish({ ok: false, error: "pipe timeout" }), 3000);
  });
}
