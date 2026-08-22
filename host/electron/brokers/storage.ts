import { assertContext, type BrokerTransport, type LaunchContext } from "./types";

export type StorageRequest =
  | { kind: "read"; rootCapabilityId: string; relativePath: string; maxBytes: number }
  | {
      kind: "write";
      rootCapabilityId: string;
      relativePath: string;
      bytes: Uint8Array;
      expectedDigest?: string;
    }
  | { kind: "list"; rootCapabilityId: string; relativePath: string; cursor?: string; limit: number }
  | { kind: "delete"; rootCapabilityId: string; relativePath: string; expectedDigest?: string };
export type StorageResponse = { bytes?: Uint8Array; entries?: string[]; digest?: string };
export class StorageBroker {
  constructor(private readonly transport: BrokerTransport<StorageRequest, StorageResponse>) {}
  request(context: LaunchContext, request: StorageRequest) {
    assertContext(context, `storage.${request.kind}`);
    return this.transport(context, request);
  }
}
