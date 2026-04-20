import { spawn, type ChildProcessWithoutNullStreams } from "node:child_process";
import fs from "node:fs";
import path from "node:path";
import { app } from "electron";

interface ArkRequest {
  operation: string;
  [key: string]: unknown;
}

interface ArkResponse<T> {
  ok: boolean;
  data?: T;
  error?: string;
}

interface ArkEvent {
  event: string;
  [key: string]: unknown;
}

interface PendingRequest<T> {
  request: ArkRequest;
  resolve: (value: T) => void;
  reject: (error: Error) => void;
}

export interface ArkObjectRecord {
  id: string;
  typeId: string;
  title: string;
  contentJson: unknown;
  propsJson: Record<string, unknown>;
  createdAt: string;
  updatedAt: string;
  deletedAt?: string | null;
}

export interface ArkObjectTypeRecord {
  id: string;
  name: string;
  schemaJson: string;
  uiSchemaJson: string;
  createdAt: string;
  updatedAt: string;
  systemLocked: boolean;
}

export interface ArkObjectLinkRecord {
  id: string;
  sourceObjectId: string;
  targetObjectId: string;
  linkType: string;
  createdAt: string;
}

function getArkBinaryPath() {
  const binaryName = process.platform === "win32" ? "ark-core-rpc.exe" : "ark-core-rpc";

  if (app.isPackaged) {
    return path.join(process.resourcesPath, "ark-core", binaryName);
  }

  const repoRoot = path.resolve(process.env.APP_ROOT ?? process.cwd(), "..", "..", "..");
  const releasePath = path.join(repoRoot, "packages", "ark-core", "rust", "target", "release", binaryName);
  if (fs.existsSync(releasePath)) {
    return releasePath;
  }

  return path.join(repoRoot, "packages", "ark-core", "rust", "target", "debug", binaryName);
}

export function getArkDbPath() {
  const override = process.env.ARK_DB_PATH?.trim();
  if (override) {
    return path.resolve(override);
  }

  return path.join(app.getPath("appData"), "Kepler", "ark.db");
}

class ArkClient {
  private child: ChildProcessWithoutNullStreams | null = null;
  private stdoutBuffer = "";
  private stderrBuffer = "";
  private requestQueue: Array<PendingRequest<unknown>> = [];
  private activeRequest: PendingRequest<unknown> | null = null;
  private initialized = false;

  private ensureChild() {
    if (this.child) {
      return this.child;
    }

    const child = spawn(getArkBinaryPath(), [], {
      stdio: ["pipe", "pipe", "pipe"],
    });

    child.stdout.setEncoding("utf8");
    child.stderr.setEncoding("utf8");

    child.stdout.on("data", (chunk: string) => {
      this.stdoutBuffer += chunk;
      this.flushStdout();
    });

    child.stderr.on("data", (chunk: string) => {
      this.stderrBuffer += chunk;
    });

    child.on("error", (error) => {
      this.failAll(error instanceof Error ? error : new Error(String(error)));
    });

    child.on("close", (code) => {
      const reason = this.stderrBuffer.trim() || `ark-core-rpc exited with ${code}`;
      this.failAll(new Error(reason));
    });

    this.child = child;
    this.stdoutBuffer = "";
    this.stderrBuffer = "";

    if (!this.initialized) {
      const dbPath = getArkDbPath();
      fs.mkdirSync(path.dirname(dbPath), { recursive: true });
      child.stdin.write(`${JSON.stringify({ operation: "init", dbPath })}\n`);
    }

    return child;
  }

  private isArkEventMessage(value: unknown): value is ArkEvent {
    return Boolean(
      value &&
      typeof value === "object" &&
      "event" in value &&
      typeof (value as { event?: unknown }).event === "string",
    );
  }

  private settleInit(response: ArkResponse<unknown>) {
    if (!response.ok) {
      const error = new Error(response.error || "ark-core-rpc init failed");
      this.initialized = false;
      this.failAll(error);
      return false;
    }

    this.initialized = true;
    this.dispatchNext();
    return true;
  }

  private flushStdout() {
    while (true) {
      const newlineIndex = this.stdoutBuffer.indexOf("\n");
      if (newlineIndex === -1) {
        return;
      }

      const rawLine = this.stdoutBuffer.slice(0, newlineIndex).trim();
      this.stdoutBuffer = this.stdoutBuffer.slice(newlineIndex + 1);

      if (!rawLine) {
        continue;
      }

      let parsed: ArkResponse<unknown> | ArkEvent;
      try {
        parsed = JSON.parse(rawLine) as ArkResponse<unknown> | ArkEvent;
      } catch (error) {
        const parseError = error instanceof Error ? error : new Error(String(error));
        if (!this.initialized) {
          this.failAll(parseError);
          return;
        }
        const pendingWithParseError = this.activeRequest;
        this.activeRequest = null;
        pendingWithParseError?.reject(parseError);
        this.dispatchNext();
        continue;
      }

      if (this.isArkEventMessage(parsed)) {
        continue;
      }

      if (!this.initialized) {
        if (!this.settleInit(parsed)) {
          return;
        }
        continue;
      }

      const pending = this.activeRequest;
      this.activeRequest = null;

      if (!pending) {
        continue;
      }

      if (!parsed.ok) {
        pending.reject(new Error(parsed.error || "ark-core-rpc request failed"));
      } else {
        pending.resolve(parsed.data);
      }

      this.dispatchNext();
    }
  }

  private dispatchNext() {
    if (this.activeRequest || this.requestQueue.length === 0) {
      return;
    }

    const child = this.ensureChild();

    if (!this.initialized) {
      return;
    }

    const nextRequest = this.requestQueue.shift();
    if (!nextRequest) {
      return;
    }

    this.activeRequest = nextRequest;

    try {
      child.stdin.write(`${JSON.stringify(nextRequest.request)}\n`);
    } catch (error) {
      this.activeRequest = null;
      nextRequest.reject(error instanceof Error ? error : new Error(String(error)));
      this.resetChild();
      this.dispatchNext();
    }
  }

  private failAll(error: Error) {
    const pending = this.activeRequest;
    const queued = this.requestQueue.splice(0);
    this.activeRequest = null;
    this.resetChild();
    if (pending) {
      pending.reject(error);
    }
    queued.forEach((request) => request.reject(error));
  }

  private resetChild() {
    if (this.child) {
      this.child.removeAllListeners();
      this.child.stdout.removeAllListeners();
      this.child.stderr.removeAllListeners();
      if (!this.child.killed) {
        this.child.kill();
      }
    }
    this.child = null;
    this.stdoutBuffer = "";
    this.stderrBuffer = "";
    this.initialized = false;
  }

  request<T>(request: ArkRequest): Promise<T> {
    return new Promise<T>((resolve, reject) => {
      this.requestQueue.push({
        request,
        resolve: resolve as (value: unknown) => void,
        reject,
      });
      this.dispatchNext();
    });
  }

  shutdown() {
    this.requestQueue.splice(0).forEach((request) => {
      request.reject(new Error("ark-core-rpc client shut down"));
    });

    if (this.activeRequest) {
      this.activeRequest.reject(new Error("ark-core-rpc client shut down"));
      this.activeRequest = null;
    }

    this.resetChild();
  }
}

const arkClient = new ArkClient();

export function runArkRequest<T>(request: ArkRequest) {
  return arkClient.request<T>(request);
}

export function shutdownArk() {
  arkClient.shutdown();
}
