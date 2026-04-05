// ArkSync browser-safe exports — NO Node.js modules (crypto, ws, os)

export { HLC } from "./src/hlc";

export {
  LAN_SYNC_PORT,
  PROTOCOL_VERSION,
  MAX_BATCH_SIZE,
  MAX_BATCH_BYTES,
  BATCH_ACK_TIMEOUT_MS,
  LIVE_ACK_TIMEOUT_MS,
  MAX_RETRIES,
  PING_INTERVAL_MS,
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
