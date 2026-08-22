import { assertContext, type BrokerTransport, type LaunchContext } from "./types";
export type WindowRequest = { kind: "perform"; action: "minimize" | "maximize" | "close" };
export type WindowResponse = { ok: true };
export class WindowBroker {
  constructor(private readonly transport: BrokerTransport<WindowRequest, WindowResponse>) {}
  request(context: LaunchContext, request: WindowRequest) {
    assertContext(context, `window.${request.action}`);
    return this.transport(context, request);
  }
}
