/**
 * LAN Sync Protocol — shared types and utilities for P2P sync over WebSocket.
 *
 * Hub model: Electron = WS server (port 21531), Android = WS client.
 * Protocol flow: hello → version_vector → sync_changes (batches + ACK) → live mode.
 */

import { HLC } from "./hlc";

// ---------------------------------------------------------------------------
// Constants
// ---------------------------------------------------------------------------

export const LAN_SYNC_PORT = 21531;
export const PROTOCOL_VERSION = 1;
export const MAX_BATCH_SIZE = 100;
export const MAX_BATCH_BYTES = 1_048_576; // 1 MB
export const BATCH_ACK_TIMEOUT_MS = 10_000;
export const LIVE_ACK_TIMEOUT_MS = 5_000;
export const MAX_RETRIES = 3;
export const PING_INTERVAL_MS = 15_000;

// ---------------------------------------------------------------------------
// Entity types that sync
// ---------------------------------------------------------------------------

export type SyncEntityType = "todo" | "project" | "area" | "tag" | "heading";

export interface SyncEntity {
  type: SyncEntityType;
  id: string;
  data: Record<string, unknown>;
  hlc: string; // HLC string: "ISO:COUNTER:DEVICE_ID"
  deleted?: boolean;
}

// ---------------------------------------------------------------------------
// Version vector: entity_id → HLC string
// ---------------------------------------------------------------------------

export type VersionVector = Record<string, string>;

// ---------------------------------------------------------------------------
// Protocol messages
// ---------------------------------------------------------------------------

export interface HelloMessage {
  type: "hello";
  protocol_version: number;
  device_id: string;
  device_name: string;
  space_id: string;
  /** Self-announced addresses (e.g. ["192.168.1.70:21531", "[fe80::1%en0]:21531"]) */
  addresses?: string[];
}

export interface VersionVectorMessage {
  type: "version_vector";
  vector: VersionVector;
}

export interface SyncChangesMessage {
  type: "sync_changes";
  batch_id: string;
  entities: SyncEntity[];
  is_last: boolean; // true if this is the final batch
}

export interface SyncAckMessage {
  type: "sync_ack";
  batch_id: string;
  accepted: number; // count of entities accepted
}

export interface LiveChangeMessage {
  type: "live_change";
  change_id: string;
  entity: SyncEntity;
}

export interface LiveAckMessage {
  type: "live_ack";
  change_id: string;
}

export interface PingMessage {
  type: "ping";
  ts: number;
}

export interface PongMessage {
  type: "pong";
  ts: number;
}

// ---------------------------------------------------------------------------
// Peer records (multi-address, Syncthing-style)
// ---------------------------------------------------------------------------

export interface PeerRecord {
  device_id: string;
  device_name: string;
  /** All known addresses for this peer (e.g. "192.168.1.70:21531") */
  addresses: string[];
  last_seen: string; // ISO 8601
  /** The address that last successfully connected */
  last_address?: string;
}

export interface PeerListMessage {
  type: "peer_list";
  peers: PeerRecord[];
}

export type LanSyncMessage =
  | HelloMessage
  | VersionVectorMessage
  | SyncChangesMessage
  | SyncAckMessage
  | LiveChangeMessage
  | LiveAckMessage
  | PingMessage
  | PongMessage
  | PeerListMessage;

// ---------------------------------------------------------------------------
// HLC comparison
// ---------------------------------------------------------------------------

/**
 * Compare two HLC strings. Returns negative if a < b, 0 if equal, positive if a > b.
 * This is the core conflict resolution primitive — LWW (Last Writer Wins).
 */
export function compareHlc(a: string, b: string): number {
  const hlcA = HLC.fromString(a);
  const hlcB = HLC.fromString(b);
  return hlcA.compareTo(hlcB);
}

/**
 * Returns true if HLC `a` is newer (greater) than HLC `b`.
 */
export function isNewerHlc(a: string, b: string): boolean {
  return compareHlc(a, b) > 0;
}

// ---------------------------------------------------------------------------
// Version vector diff
// ---------------------------------------------------------------------------

/**
 * Compute which entity IDs from `remote` are missing or outdated in `local`.
 *
 * Returns the set of entity IDs that the local side needs from the remote.
 */
export function computeVectorDiff(
  local: VersionVector,
  remote: VersionVector,
): Set<string> {
  const needed = new Set<string>();

  for (const [entityId, remoteHlc] of Object.entries(remote)) {
    const localHlc = local[entityId];
    if (!localHlc) {
      // Entity doesn't exist locally
      needed.add(entityId);
    } else if (isNewerHlc(remoteHlc, localHlc)) {
      // Remote has a newer version
      needed.add(entityId);
    }
  }

  return needed;
}

/**
 * Compute which entity IDs the local side has that the remote doesn't (or has older).
 * This is effectively `computeVectorDiff(remote, local)`.
 */
export function computeLocalExcess(
  local: VersionVector,
  remote: VersionVector,
): Set<string> {
  return computeVectorDiff(remote, local);
}

// ---------------------------------------------------------------------------
// Batch splitting
// ---------------------------------------------------------------------------

/**
 * Split an array of SyncEntity into batches respecting MAX_BATCH_SIZE
 * and MAX_BATCH_BYTES constraints.
 */
export function splitIntoBatches(entities: SyncEntity[]): SyncEntity[][] {
  if (entities.length === 0) return [];

  const batches: SyncEntity[][] = [];
  let current: SyncEntity[] = [];
  let currentBytes = 0;

  for (const entity of entities) {
    const entityBytes = JSON.stringify(entity).length;

    // If adding this entity would exceed limits, start a new batch
    // (unless current batch is empty — always allow at least one entity per batch)
    if (
      current.length > 0 &&
      (current.length >= MAX_BATCH_SIZE ||
        currentBytes + entityBytes > MAX_BATCH_BYTES)
    ) {
      batches.push(current);
      current = [];
      currentBytes = 0;
    }

    current.push(entity);
    currentBytes += entityBytes;
  }

  if (current.length > 0) {
    batches.push(current);
  }

  return batches;
}

// ---------------------------------------------------------------------------
// Message serialization helpers
// ---------------------------------------------------------------------------

export function serializeMessage(msg: LanSyncMessage): string {
  return JSON.stringify(msg);
}

export function deserializeMessage(raw: string): LanSyncMessage | null {
  try {
    const parsed = JSON.parse(raw);
    if (typeof parsed !== "object" || parsed === null || !parsed.type) {
      return null;
    }
    return parsed as LanSyncMessage;
  } catch {
    return null;
  }
}

/**
 * Generate a unique batch/change ID.
 */
export function generateId(): string {
  return `${Date.now()}-${Math.random().toString(36).slice(2, 8)}`;
}

// ---------------------------------------------------------------------------
// Peer record merging
// ---------------------------------------------------------------------------

/**
 * Merge incoming peer records into existing ones.
 * - Union merge of addresses (no duplicates)
 * - Preserve the newest last_seen
 * - Preserve last_address from whichever record is newer
 * - New peers are appended
 */
export function mergePeerRecords(
  existing: PeerRecord[],
  incoming: PeerRecord[],
): PeerRecord[] {
  const map = new Map<string, PeerRecord>();

  // Index existing records
  for (const peer of existing) {
    map.set(peer.device_id, { ...peer });
  }

  // Merge incoming
  for (const inc of incoming) {
    const current = map.get(inc.device_id);
    if (!current) {
      map.set(inc.device_id, { ...inc });
      continue;
    }

    // Union merge addresses
    const addrSet = new Set([...current.addresses, ...inc.addresses]);
    current.addresses = [...addrSet];

    // Keep newest last_seen
    if (inc.last_seen > current.last_seen) {
      current.last_seen = inc.last_seen;
      current.device_name = inc.device_name;
      if (inc.last_address) {
        current.last_address = inc.last_address;
      }
    }
  }

  return [...map.values()];
}
