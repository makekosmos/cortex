/**
 * Legacy Electron UDP broadcast discovery. The runtime implementation has
 * moved into the Rust `ark-core` crate (`packages/ark-core/rust/src/beacon.rs`)
 * and the Electron main process talks to it via the `ark-core-rpc` sidecar
 * (see `./sidecar.ts`). This file is retained only as an exported type
 * module so any lingering type imports keep compiling. No runtime class is
 * instantiated here anymore.
 *
 * See the spec at `.agent/tasks/ark-rust-runtime/spec.md` (AC8) for the
 * migration details.
 */

/** Peer identifier + address snapshot as seen in a UDP beacon. */
export interface BeaconPeer {
  deviceId: string;
  deviceName: string;
  /** Primary `ip:wsPort` (the sender UDP source address). */
  address: string;
  /** All routable addresses advertised by the beacon. */
  addresses: string[];
}

/** Options previously passed to the removed `BroadcastDiscovery` class. */
export interface BroadcastDiscoveryOptions {
  spaceId: string;
  deviceId: string;
  deviceName: string;
  wsPort: number;
  onPeerDiscovered: (peer: BeaconPeer) => void;
}
