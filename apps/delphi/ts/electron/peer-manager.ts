/**
 * Coordinates PeerServer, PeerDiscovery, and outbound connections.
 *
 * Lifecycle:
 *   1. Start PeerServer on a random port
 *   2. Advertise via mDNS
 *   3. Browse for peers with the same mesh_id
 *   4. Connect to discovered peers
 *   5. Relay changes between all connections
 */

import WebSocket from 'ws';
import { PeerServer, type ChangeHandler } from './peer-server';
import { PeerDiscovery, type DiscoveredPeer } from './peer-discovery';
import {
  createPeerHello,
  verifyAuthHmac,
  computeMeshId,
  type PeerChange,
  type PeerHelloAck,
} from '../src/services/sync/peer-protocol';

const RECONNECT_INTERVAL_MS = 15_000;

export class PeerManager {
  private server: PeerServer;
  private discovery: PeerDiscovery;
  private outbound: Map<string, WebSocket> = new Map();
  private meshSecret: string;
  private meshId: string;
  private deviceId: string;
  private deviceName: string;
  private platform: string;
  private reconnectTimers: Map<string, ReturnType<typeof setTimeout>> = new Map();
  private stopped = false;

  private onChangeHandler: ChangeHandler | null = null;

  constructor(opts: {
    meshSecret: string;
    deviceId: string;
    deviceName: string;
    platform: string;
  }) {
    this.meshSecret = opts.meshSecret;
    this.meshId = computeMeshId(opts.meshSecret);
    this.deviceId = opts.deviceId;
    this.deviceName = opts.deviceName;
    this.platform = opts.platform;

    this.server = new PeerServer(opts.meshSecret, opts.deviceId, opts.deviceName);
    this.discovery = new PeerDiscovery(
      (peer) => this.connectToPeer(peer),
      (deviceId) => this.handlePeerLost(deviceId),
    );

    // Forward inbound changes
    this.server.onChange((change, fromDevice) => {
      if (this.onChangeHandler) {
        this.onChangeHandler(change, fromDevice);
      }
    });
  }

  /** Register a handler for incoming changes from any peer. */
  onChange(handler: ChangeHandler): void {
    this.onChangeHandler = handler;
  }

  async start(): Promise<void> {
    this.stopped = false;
    const port = await this.server.start();

    this.discovery.advertise({
      deviceId: this.deviceId,
      deviceName: this.deviceName,
      platform: this.platform,
      meshId: this.meshId,
      port,
    });

    this.discovery.browse(this.meshId);
    console.log(`[PeerManager] Started (port=${port}, mesh=${this.meshId.slice(0, 8)}...)`);
  }

  stop(): void {
    this.stopped = true;

    // Clear all reconnect timers
    for (const timer of this.reconnectTimers.values()) {
      clearTimeout(timer);
    }
    this.reconnectTimers.clear();

    // Close all outbound connections
    for (const [deviceId, ws] of this.outbound) {
      try {
        ws.close(1000, 'manager stopping');
      } catch {
        // ignore
      }
      this.outbound.delete(deviceId);
    }

    this.discovery.stop();
    this.server.stop();
    console.log('[PeerManager] Stopped');
  }

  /** Broadcast a change to all peers (inbound and outbound). */
  broadcastChange(change: PeerChange): void {
    // Add ourselves to hop path
    if (!change.hop_path.includes(this.deviceId)) {
      change.hop_path.push(this.deviceId);
    }

    // Broadcast via server (to inbound connections)
    this.server.broadcastChange(change);

    // Send to all outbound connections
    const msg = JSON.stringify(change);
    for (const [deviceId, ws] of this.outbound) {
      if (change.hop_path.includes(deviceId)) continue;
      if (ws.readyState === WebSocket.OPEN) {
        ws.send(msg);
      }
    }
  }

  // ---------------------------------------------------------------------------
  // Internal
  // ---------------------------------------------------------------------------

  private connectToPeer(peer: DiscoveredPeer): void {
    // Prevent self-connection
    if (peer.deviceId === this.deviceId) return;

    // Prevent duplicate connection
    if (this.outbound.has(peer.deviceId)) return;

    // Also skip if server already has an inbound from this device
    if (this.server.getConnectedDevices().includes(peer.deviceId)) return;

    console.log(`[PeerManager] Connecting to ${peer.deviceName} at ${peer.host}:${peer.port}`);

    const ws = new WebSocket(`ws://${peer.host}:${peer.port}`);
    let authenticated = false;

    ws.on('open', () => {
      const hello = createPeerHello(this.deviceId, this.deviceName, this.platform, this.meshSecret);
      ws.send(JSON.stringify(hello));
    });

    ws.on('message', (raw) => {
      let msg: Record<string, unknown>;
      try {
        msg = JSON.parse(raw.toString());
      } catch {
        return;
      }

      if (!authenticated) {
        if (msg.type !== 'peer_hello_ack') return;
        const ack = msg as unknown as PeerHelloAck;
        if (!ack.ok) {
          console.warn(`[PeerManager] Peer rejected connection: ${ack.error}`);
          ws.close();
          return;
        }
        // Verify the ack's HMAC
        if (!verifyAuthHmac(this.meshSecret, ack.nonce, ack.auth_hmac)) {
          console.warn('[PeerManager] Peer ack HMAC verification failed');
          ws.close();
          return;
        }
        authenticated = true;
        this.outbound.set(peer.deviceId, ws);
        console.log(`[PeerManager] Connected to peer: ${peer.deviceName}`);
        return;
      }

      // Handle sync messages
      if (msg.type === 'change') {
        const change = msg as unknown as PeerChange;
        if (!change.hop_path.includes(this.deviceId)) {
          change.hop_path.push(this.deviceId);
        }
        if (this.onChangeHandler) {
          this.onChangeHandler(change, peer.deviceId);
        }
        // Re-broadcast to other peers
        this.server.broadcastChange(change, peer.deviceId);
        for (const [otherDeviceId, otherWs] of this.outbound) {
          if (otherDeviceId === peer.deviceId) continue;
          if (change.hop_path.includes(otherDeviceId)) continue;
          if (otherWs.readyState === WebSocket.OPEN) {
            otherWs.send(JSON.stringify(change));
          }
        }
      }
    });

    ws.on('close', () => {
      this.outbound.delete(peer.deviceId);
      if (authenticated) {
        console.log(`[PeerManager] Disconnected from peer: ${peer.deviceName}`);
      }
      this.scheduleReconnect(peer);
    });

    ws.on('error', (err) => {
      console.error(`[PeerManager] Connection error (${peer.deviceName}):`, err.message);
    });
  }

  private handlePeerLost(deviceId: string): void {
    const ws = this.outbound.get(deviceId);
    if (ws) {
      try {
        ws.close(1000, 'peer lost');
      } catch {
        // ignore
      }
      this.outbound.delete(deviceId);
    }
    // Cancel reconnect timer for lost peer
    const timer = this.reconnectTimers.get(deviceId);
    if (timer) {
      clearTimeout(timer);
      this.reconnectTimers.delete(deviceId);
    }
  }

  private scheduleReconnect(peer: DiscoveredPeer): void {
    if (this.stopped) return;
    if (this.reconnectTimers.has(peer.deviceId)) return;

    const timer = setTimeout(() => {
      this.reconnectTimers.delete(peer.deviceId);
      if (!this.stopped) {
        this.connectToPeer(peer);
      }
    }, RECONNECT_INTERVAL_MS);

    this.reconnectTimers.set(peer.deviceId, timer);
  }
}
