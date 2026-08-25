import { assertContext, type BrokerTransport, type LaunchContext } from "./types";
type ArkValue = string | number | boolean | null | readonly ArkValue[] | { readonly [key: string]: ArkValue };
export type ArkRequest = Readonly<{ kind: string; [key: string]: ArkValue }>;
export type ArkResponse = object;
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
