import { spawn, type ChildProcessWithoutNullStreams } from "node:child_process";

import fs from "node:fs";

import path from "node:path";

import { app } from "electron";

interface HeartRequest {
  operation: string;

  [key: string]: unknown;
}

interface HeartServeResponse<T> {
  ok: boolean;

  data?: T;

  error?: string;
}

interface PendingRequest<T> {
  request: HeartRequest;

  resolve: (value: T) => void;

  reject: (error: Error) => void;
}

const MAX_STDERR_TAIL_CHARS = 32 * 1024;

function appendTail(buffer: string, chunk: string, maxChars: number) {
  const next = buffer + chunk;
  if (next.length <= maxChars) {
    return next;
  }
  return next.slice(next.length - maxChars);
}

function getHeartBinaryPath() {
  const appRoot = process.env.APP_ROOT ?? process.cwd();

  const binaryName =
    process.platform === "win32" ? "eden-heart.exe" : "eden-heart";

  if (app.isPackaged) {
    return path.join(process.resourcesPath, "eden-heart", binaryName);
  }

  const releasePath = path.join(
    appRoot,
    "heart",
    "target",
    "release",
    binaryName,
  );

  if (fs.existsSync(releasePath)) {
    return releasePath;
  }

  return path.join(appRoot, "heart", "target", "debug", binaryName);
}

class HeartClient {
  private child: ChildProcessWithoutNullStreams | null = null;

  private stdoutBuffer = "";

  private stderrBuffer = "";

  private requestQueue: Array<PendingRequest<unknown>> = [];

  private activeRequest: PendingRequest<unknown> | null = null;

  private ensureChild() {
    if (this.child) {
      return this.child;
    }

    const binaryPath = getHeartBinaryPath();

    const child = spawn(binaryPath, ["serve"], {
      stdio: ["pipe", "pipe", "pipe"],
    });

    child.stdout.setEncoding("utf8");

    child.stderr.setEncoding("utf8");

    child.stdout.on("data", (chunk: string) => {
      this.stdoutBuffer += chunk;

      this.flushStdout();
    });

    child.stderr.on("data", (chunk: string) => {
      this.stderrBuffer = appendTail(
        this.stderrBuffer,
        chunk,
        MAX_STDERR_TAIL_CHARS,
      );
    });

    child.on("error", (error) => {
      this.failAll(error instanceof Error ? error : new Error(String(error)));
    });

    child.on("close", (code) => {
      const reason =
        this.stderrBuffer.trim() || `eden-heart exited with ${code}`;

      this.failAll(new Error(reason));
    });

    this.child = child;

    this.stdoutBuffer = "";

    this.stderrBuffer = "";

    return child;
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

      const pending = this.activeRequest;

      this.activeRequest = null;

      if (!pending) {
        continue;
      }

      try {
        const response = JSON.parse(rawLine) as HeartServeResponse<unknown>;

        if (!response.ok) {
          pending.reject(
            new Error(response.error || "eden-heart request failed"),
          );
        } else {
          pending.resolve(response.data);
        }
      } catch (error) {
        pending.reject(
          error instanceof Error ? error : new Error(String(error)),
        );
      }

      this.dispatchNext();
    }
  }

  private dispatchNext() {
    if (this.activeRequest || this.requestQueue.length === 0) {
      return;
    }

    const nextRequest = this.requestQueue.shift();

    if (!nextRequest) {
      return;
    }

    const child = this.ensureChild();

    this.activeRequest = nextRequest;

    try {
      child.stdin.write(`${JSON.stringify(nextRequest.request)}\n`);
    } catch (error) {
      this.activeRequest = null;

      nextRequest.reject(
        error instanceof Error ? error : new Error(String(error)),
      );

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
  }

  request<T>(request: HeartRequest): Promise<T> {
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
      request.reject(new Error("eden-heart client shut down"));
    });

    if (this.activeRequest) {
      this.activeRequest.reject(new Error("eden-heart client shut down"));

      this.activeRequest = null;
    }

    if (this.child && !this.child.killed) {
      this.child.kill();
    }

    this.resetChild();
  }
}

const heartClient = new HeartClient();

export function runHeartRequest<T>(request: HeartRequest): Promise<T> {
  return heartClient.request<T>(request);
}

export function shutdownHeart() {
  heartClient.shutdown();
}
