import { ArkBroker } from "./ark";
import { DialogBroker } from "./dialogs";
import { NetworkBroker } from "./network";
import { StorageBroker } from "./storage";
import { WindowBroker } from "./window";
import type { LaunchContext } from "./types";

export type HostBrokers = Readonly<{
  ark: ArkBroker;
  storage: StorageBroker;
  network: NetworkBroker;
  dialog: DialogBroker;
  window: WindowBroker;
}>;
export function createHostBrokers(
  context: LaunchContext,
  transport: {
    ark: ConstructorParameters<typeof ArkBroker>[0];
    storage: ConstructorParameters<typeof StorageBroker>[0];
    network: ConstructorParameters<typeof NetworkBroker>[0];
    dialog: ConstructorParameters<typeof DialogBroker>[0];
    window: ConstructorParameters<typeof WindowBroker>[0];
  },
): HostBrokers {
  if (!context.launchId || !context.packageId || !context.sessionId)
    throw new Error("SESSION_DENIED:host");
  return {
    ark: new ArkBroker(transport.ark),
    storage: new StorageBroker(transport.storage),
    network: new NetworkBroker(transport.network),
    dialog: new DialogBroker(transport.dialog),
    window: new WindowBroker(transport.window),
  };
}
