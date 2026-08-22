import { assertContext, type BrokerTransport, type LaunchContext } from "./types";
export type DialogRequest = {
  kind: "open";
  dialogKind: string;
  options: Readonly<Record<string, string>>;
};
export type DialogResponse = { accepted: boolean; value?: string };
export class DialogBroker {
  constructor(private readonly transport: BrokerTransport<DialogRequest, DialogResponse>) {}
  request(context: LaunchContext, request: DialogRequest) {
    assertContext(context, "dialog.open");
    return this.transport(context, request);
  }
}
