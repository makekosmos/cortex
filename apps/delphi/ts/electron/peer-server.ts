/**
 * P2P WebSocket server that runs in the Electron main process.
 *
 * Accepts inbound connections from LAN peers, authenticates them
 * via the mesh secret HMAC handshake, and relays changes.
 */

import { WebSocketServer, WebSocket } from 'ws';
import {
  createPeerHelloAck,
  verifyAuthHmac,
  computeMeshId,
  type PeerHello,
  type PeerChange,
} from '../src/services/sync/peer-protocol';

const HANDSHAKE_TIMEOUT_MS = 10_000;

export type ChangeHandler = (change: PeerChange, fromDevice: string) => void;

export class PeerServer {
  private wss: WebSocketServer | null = null;
  private port = 0;
  private meshSecret: string;
  private deviceId: string;
  private deviceName: string;
  private connections: Map<string, WebSocket> = new Map();
  private onChangeHandler: ChangeHandler | null = null;
  private onPeerConnectHandler: ((deviceId: string) => void) | null = null;
  private onPeerDisconnectHandler: ((deviceId: string) => void) | null = null;

  constructor(meshSecret: string, deviceId: string, deviceName: string) {
    this.meshSecret = meshSecret;
    this.deviceId = deviceId;
    this.deviceName = deviceName;
  }

  onChange(handler: ChangeHandler): void {
    this.onChangeHandler = handler;
  }

  onPeerConnect(handler: (deviceId: string) => void): void {
    this.onPeerConnectHandler = handler;
  }

  onPeerDisconnect(handler: (deviceId: string) => void): void {
    this.onPeerDisconnectHandler = handler;
  }

  async start(preferredPort = 0): Promise<number> {
    return new Promise((resolve, reject) => {
      this.wss = new WebSocketServer({ port: preferredPort });

      this.wss.on('listening', () => {
        const addr = this.wss!.address();
        this.port = typeof addr === 'object' ? addr.port : preferredPort;
        console.log(`[PeerServer] Listening on port ${this.port}`);
        resolve(this.port);
      });

      this.wss.on('error', (err) => {
        console.error('[PeerServer] Server error:', err);
        reject(err);
      });

      this.wss.on('connection', (ws) => this.handleConnection(ws));
    });
  }

  stop(): void {
    for (const ws of this.connections.values()) {
      try {
        ws.close(1000, 'server stopping');
      } catch {
        // ignore
      }
    }
    this.connections.clear();

    if (this.wss) {
      this.wss.close();
      this.wss = null;
    }
    this.port = 0;
    console.log('[PeerServer] Stopped');
  }

  getPort(): number {
    return this.port;
  }

  getConnectedDevices(): string[] {
    return Array.from(this.connections.keys());
  }

  /** Broadcast a change to all connected peers (with loop prevention). */
  broadcastChange(change: PeerChange, excludeDevice?: string): void {
    const msg = JSON.stringify(change);
    for (const [deviceId, ws] of this.connections) {
      if (deviceId === excludeDevice) continue;
      if (change.hop_path.includes(deviceId)) continue;
      if (ws.readyState === WebSocket.OPEN) {
        ws.send(msg);
      }
    }
  }

  // ---------------------------------------------------------------------------
  // Internal
  // ---------------------------------------------------------------------------

  private handleConnection(ws: WebSocket): void {
    let authenticated = false;
    let peerDeviceId = '';

    const timeout = setTimeout(() => {
      if (!authenticated) {
        console.warn('[PeerServer] Handshake timeout, closing');
        ws.close(4001, 'handshake timeout');
      }
    }, HANDSHAKE_TIMEOUT_MS);

    ws.on('message', (raw) => {
      let msg: Record<string, unknown>;
      try {
        msg = JSON.parse(raw.toString());
      } catch {
        return;
      }

      if (!authenticated) {
        if (msg.type !== 'peer_hello') {
          ws.close(4002, 'expected peer_hello');
          clearTimeout(timeout);
          return;
        }
        this.handleHello(ws, msg as unknown as PeerHello, timeout).then(
          (deviceId) => {
            if (deviceId) {
              authenticated = true;
              peerDeviceId = deviceId;
            }
          },
        );
        return;
      }

      // Authenticated — handle sync messages
      if (msg.type === 'change') {
        const change = msg as unknown as PeerChange;
        // Add ourselves to hop path if not already there
        if (!change.hop_path.includes(this.deviceId)) {
          change.hop_path.push(this.deviceId);
        }
        if (this.onChangeHandler) {
          this.onChangeHandler(change, peerDeviceId);
        }
        // Re-broadcast to other peers
        this.broadcastChange(change, peerDeviceId);
      }
    });

    ws.on('close', () => {
      clearTimeout(timeout);
      if (peerDeviceId) {
        this.connections.delete(peerDeviceId);
        console.log(`[PeerServer] Peer disconnected: ${peerDeviceId}`);
        this.onPeerDisconnectHandler?.(peerDeviceId);
      }
    });

    ws.on('error', (err) => {
      console.error(`[PeerServer] Connection error (${peerDeviceId}):`, err.message);
    });
  }

  private async handleHello(
    ws: WebSocket,
    hello: PeerHello,
    timeout: ReturnType<typeof setTimeout>,
  ): Promise<string | null> {
    clearTimeout(timeout);

    // Verify mesh ID matches
    const expectedMeshId = computeMeshId(this.meshSecret);
    if (hello.mesh_id !== expectedMeshId) {
      const ack = createPeerHelloAck(false, this.deviceId, this.deviceName, 'electron', this.meshSecret, 'mesh_id mismatch');
      ws.send(JSON.stringify(ack));
      ws.close(4003, 'mesh_id mismatch');
      return null;
    }

    // Verify HMAC
    if (!verifyAuthHmac(this.meshSecret, hello.nonce, hello.auth_hmac)) {
      const ack = createPeerHelloAck(false, this.deviceId, this.deviceName, 'electron', this.meshSecret, 'auth failed');
      ws.send(JSON.stringify(ack));
      ws.close(4004, 'auth failed');
      return null;
    }

    // Prevent self-connection
    if (hello.device_id === this.deviceId) {
      ws.close(4005, 'self connection');
      return null;
    }

    // Close any existing connection from the same device
    const existing = this.connections.get(hello.device_id);
    if (existing) {
      try {
        existing.close(1000, 'replaced');
      } catch {
        // ignore
      }
    }

    this.connections.set(hello.device_id, ws);
    console.log(`[PeerServer] Authenticated peer: ${hello.device_id} (${hello.device_name})`);
    this.onPeerConnectHandler?.(hello.device_id);

    const ack = createPeerHelloAck(true, this.deviceId, this.deviceName, 'electron', this.meshSecret);
    ws.send(JSON.stringify(ack));

    return hello.device_id;
  }
}
