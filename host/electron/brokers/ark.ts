import { assertContext, type BrokerTransport, type LaunchContext } from "./types";
export type ArkRequest = Readonly<Record<string, unknown>> & { kind: string };
export type ArkResponse = unknown;
export class ArkBroker {
  constructor(private readonly transport: BrokerTransport<ArkRequest, ArkResponse>) {}
  request(context: LaunchContext, request: ArkRequest) {
    assertContext(context, `ark.${request.kind}`);
    if (!request.kind || request.kind === "raw" || "operation" in request || "params" in request) {
      throw new Error("CONTRACT_INVALID:ark");
    }
    return this.transport(context, request);
  }
}
