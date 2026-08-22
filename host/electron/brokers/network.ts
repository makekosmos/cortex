import { assertContext, type BrokerTransport, type LaunchContext } from "./types";
export type NetworkRequest = {
  kind: "fetch";
  url: string;
  method: string;
  headers: Record<string, string>;
  body?: Uint8Array;
  redirectMode: "error" | "manual";
};
export type NetworkResponse = { status: number; headers: Record<string, string>; body: Uint8Array };
export class NetworkBroker {
  constructor(private readonly transport: BrokerTransport<NetworkRequest, NetworkResponse>) {}
  request(context: LaunchContext, request: NetworkRequest) {
    assertContext(context, "network.fetch");
    return this.transport(context, request);
  }
}
