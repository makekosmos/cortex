export type LaunchContext = Readonly<{
  launchId: string;
  packageId: string;
  packageVersion: string;
  manifestDigest: string;
  sessionId: string;
  origin: string;
  generation: number;
}>;

export type BrokerErrorCode =
  | "CAPABILITY_DENIED"
  | "ORIGIN_DENIED"
  | "SESSION_DENIED"
  | "GRANT_REVOKED"
  | "CONTRACT_INVALID";

export type BrokerError = Readonly<{
  category: "permission" | "lifecycle" | "contract";
  code: BrokerErrorCode;
  operation: string;
  retryable: boolean;
}>;

export class HostBrokerError extends Error {
  readonly category: BrokerError["category"];
  readonly code: BrokerErrorCode;
  readonly operation: string;
  readonly retryable: boolean;

  constructor(error: BrokerError) {
    super(`${error.code}:${error.operation}`);
    this.name = "HostBrokerError";
    this.category = error.category;
    this.code = error.code;
    this.operation = error.operation;
    this.retryable = error.retryable;
  }
}

export type BrokerTransport<Request, Response> = (
  context: LaunchContext,
  request: Request,
) => Promise<Response>;

export function deny(code: BrokerErrorCode, operation: string): never {
  throw new HostBrokerError({
    category: code === "CONTRACT_INVALID" ? "contract" : "permission",
    code,
    operation,
    retryable: false,
  });
}

export function assertContext(context: LaunchContext, operation: string): void {
  if (!context.launchId || !context.packageId || !context.sessionId || context.generation < 0) {
    deny("SESSION_DENIED", operation);
  }
  if (!/^https?:\/\//.test(context.origin)) deny("ORIGIN_DENIED", operation);
}
