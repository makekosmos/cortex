/**
 * UDP Broadcast-based peer discovery for LAN sync.
 *
 * Unlike mDNS (which uses multicast and is often blocked by routers),
 * UDP broadcast works on virtually all LANs. Each peer periodically
 * sends a beacon on UDP port LAN_SYNC_PORT+1 containing its space_id
 * (hashed, safe to broadcast) and WS server port. Listeners on the
 * same space respond by updating known peer addresses.
 *
 * Beacon format (JSON):
 *   { "t": "delphi", "s": "<space_id>", "d": "<device_id>", "n": "<device_name>", "p": <ws_port> }
 */

import dgram from 'node:dgram';
import os from 'node:os';
import { LAN_SYNC_PORT } from '../src/services/sync/lan-protocol';

const TAG = '[BroadcastDiscovery]';
const BEACON_PORT = LAN_SYNC_PORT + 1; // 21532
const BEACON_INTERVAL_MS = 5_000;
const BEACON_TYPE = 'delphi';

export interface BeaconPeer {
  deviceId: string;
  deviceName: string;
  address: string; // "ip:wsPort"
}

export interface BroadcastDiscoveryOptions {
  spaceId: string;
  deviceId: string;
  deviceName: string;
  wsPort: number;
  onPeerDiscovered: (peer: BeaconPeer) => void;
}

export class BroadcastDiscovery {
  private socket: dgram.Socket | null = null;
  private sendTimer: ReturnType<typeof setInterval> | null = null;
  private options: BroadcastDiscoveryOptions;
  private stopped = false;

  constructor(options: BroadcastDiscoveryOptions) {
    this.options = options;
  }

  start(): void {
    this.stopped = false;

    try {
      this.socket = dgram.createSocket({ type: 'udp4', reuseAddr: true });

      this.socket.on('message', (msg, rinfo) => {
        this.handleMessage(msg, rinfo);
      });

      this.socket.on('error', (err) => {
        console.warn(`${TAG} Socket error:`, err.message);
        // Don't crash — just stop discovery
        this.stop();
      });

      this.socket.bind(BEACON_PORT, () => {
        this.socket!.setBroadcast(true);
        console.log(`${TAG} Listening on UDP ${BEACON_PORT}`);

        // Send first beacon immediately
        this.sendBeacon();

        // Then periodically
        this.sendTimer = setInterval(() => {
          this.sendBeacon();
        }, BEACON_INTERVAL_MS);
      });
    } catch (err) {
      console.warn(`${TAG} Failed to start:`, err);
    }
  }

  stop(): void {
    this.stopped = true;
    if (this.sendTimer) {
      clearInterval(this.sendTimer);
      this.sendTimer = null;
    }
    if (this.socket) {
      try { this.socket.close(); } catch { /* ignore */ }
      this.socket = null;
    }
  }

  /** Update space/device info (e.g. after space change). */
  updateOptions(opts: Partial<BroadcastDiscoveryOptions>): void {
    Object.assign(this.options, opts);
  }

  private sendBeacon(): void {
    if (this.stopped || !this.socket) return;

    const beacon = JSON.stringify({
      t: BEACON_TYPE,
      s: this.options.spaceId,
      d: this.options.deviceId,
      n: this.options.deviceName,
      p: this.options.wsPort,
    });

    const buf = Buffer.from(beacon);

    // Send to all broadcast addresses (each network interface)
    const broadcastAddrs = this.getBroadcastAddresses();
    for (const addr of broadcastAddrs) {
      try {
        this.socket.send(buf, 0, buf.length, BEACON_PORT, addr);
      } catch {
        // ignore send errors on individual interfaces
      }
    }
  }

  private handleMessage(msg: Buffer, rinfo: dgram.RemoteInfo): void {
    try {
      const data = JSON.parse(msg.toString());
      if (data.t !== BEACON_TYPE) return;
      if (data.s !== this.options.spaceId) return; // different space
      if (data.d === this.options.deviceId) return; // from self

      const peer: BeaconPeer = {
        deviceId: data.d,
        deviceName: data.n || 'Unknown',
        address: `${rinfo.address}:${data.p || LAN_SYNC_PORT}`,
      };

      this.options.onPeerDiscovered(peer);
    } catch {
      // ignore malformed messages
    }
  }

  /** Get broadcast addresses for all IPv4 interfaces. */
  private getBroadcastAddresses(): string[] {
    const addresses: string[] = ['255.255.255.255'];
    const interfaces = os.networkInterfaces();

    for (const nets of Object.values(interfaces)) {
      for (const net of nets ?? []) {
        if (net.internal || net.family !== 'IPv4') continue;
        // Calculate broadcast: ip | ~netmask
        const ipParts = net.address.split('.').map(Number);
        const maskParts = net.netmask.split('.').map(Number);
        const broadcast = ipParts.map((ip, i) => (ip | (~maskParts[i] & 0xff))).join('.');
        if (!addresses.includes(broadcast)) {
          addresses.push(broadcast);
        }
      }
    }

    return addresses;
  }
}
