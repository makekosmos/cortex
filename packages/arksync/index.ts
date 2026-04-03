// ArkSync — unified P2P sync package for kosmos apps
//
// This barrel is browser-safe. Node.js-only modules (ws, crypto, os) are
// dynamically imported inside function bodies and won't crash browser parse.

export { HLC } from "./src/hlc";

export {
  // Constants
  LAN_SYNC_PORT,
  PROTOCOL_VERSION,
  MAX_BATCH_SIZE,
  MAX_BATCH_BYTES,
  BATCH_ACK_TIMEOUT_MS,
  LIVE_ACK_TIMEOUT_MS,
  MAX_RETRIES,
  PING_INTERVAL_MS,
  // Types
  type SyncEntityType,
  type SyncEntity,
  type VersionVector,
  type HelloMessage,
  type VersionVectorMessage,
  type SyncChangesMessage,
  type SyncAckMessage,
  type LiveChangeMessage,
  type LiveAckMessage,
  type PingMessage,
  type PongMessage,
  type PeerRecord,
  type PeerListMessage,
  type LanSyncMessage,
  // Functions
  compareHlc,
  isNewerHlc,
  computeVectorDiff,
  computeLocalExcess,
  splitIntoBatches,
  serializeMessage,
  deserializeMessage,
  generateId,
  mergePeerRecords,
} from "./src/protocol";

export {
  generateSpaceCode,
  encodeIpv4,
  decodeIpv4,
  generateExtendedCode,
  formatSpaceCode,
  parseSpaceCode,
  generateQrPayload,
  parseQrPayload,
  deriveSpaceId,
} from "./src/space";

export type { StorageBackend } from "./src/storage";

// Node.js-only exports — these use `ws`, `crypto`, `os` modules.
// They are safe to re-export because the imports happen inside function/class bodies,
// not at module top level. Vite will parse but not execute them in browser context.
export {
  computeMeshId,
  computeAuthHmac,
  verifyAuthHmac,
  generateNonce,
  createPeerHello,
  createPeerHelloAck,
  type PeerHello,
  type PeerHelloAck,
  type PeerChange,
} from "./src/peer-protocol";

export { SyncServer } from "./src/sync-server";
export { SyncClient, type SyncClientOptions } from "./src/sync-client";
export { getOwnAddresses } from "./src/node";
