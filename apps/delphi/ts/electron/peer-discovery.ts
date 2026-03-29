/**
 * mDNS-based peer discovery using bonjour-service.
 *
 * Advertises this device as an `_ark-peer._tcp` service on the LAN
 * and browses for other peers with the same mesh_id.
 */

import BonjourModule, { type Service } from 'bonjour-service';

// bonjour-service exports a class as default, but the ESM interop
// may wrap it in { default: Bonjour }. Handle both cases.
const Bonjour = (typeof (BonjourModule as any).default === 'function'
  ? (BonjourModule as any).default
  : BonjourModule) as typeof BonjourModule;

export interface DiscoveredPeer {
  deviceId: string;
  deviceName: string;
  platform: string;
  host: string;
  port: number;
  meshId: string;
}

export class PeerDiscovery {
  private bonjour: InstanceType<typeof Bonjour>;
  private published = false;
  private browser: ReturnType<InstanceType<typeof Bonjour>['find']> | null = null;
  private knownPeers: Map<string, DiscoveredPeer> = new Map();
  private onPeerFound: (peer: DiscoveredPeer) => void;
  private onPeerLost: (deviceId: string) => void;

  constructor(
    onFound: (peer: DiscoveredPeer) => void,
    onLost: (deviceId: string) => void,
  ) {
    this.bonjour = new Bonjour();
    this.onPeerFound = onFound;
    this.onPeerLost = onLost;
  }

  /** Advertise ourselves on mDNS. */
  advertise(opts: {
    deviceId: string;
    deviceName: string;
    platform: string;
    meshId: string;
    port: number;
  }): void {
    if (this.published) return;

    this.bonjour.publish({
      name: opts.deviceId,
      type: 'ark-peer',
      protocol: 'tcp',
      port: opts.port,
      txt: {
        device_id: opts.deviceId,
        device_name: opts.deviceName,
        platform: opts.platform,
        mesh_id: opts.meshId,
        api_version: '2',
      },
    });

    this.published = true;
    console.log(`[PeerDiscovery] Advertising ${opts.deviceName} on port ${opts.port}`);
  }

  /** Browse for peers with a matching mesh_id. */
  browse(meshId: string): void {
    if (this.browser) return;

    this.browser = this.bonjour.find({ type: 'ark-peer', protocol: 'tcp' }, (service: Service) => {
      const txt = service.txt as Record<string, string> | undefined;
      if (!txt) return;

      if (txt.mesh_id !== meshId) return;

      const peer: DiscoveredPeer = {
        deviceId: txt.device_id || service.name,
        deviceName: txt.device_name || service.name,
        platform: txt.platform || 'unknown',
        host: service.host,
        port: service.port,
        meshId: txt.mesh_id,
      };

      // Deduplicate
      if (this.knownPeers.has(peer.deviceId)) return;

      this.knownPeers.set(peer.deviceId, peer);
      console.log(`[PeerDiscovery] Found peer: ${peer.deviceName} at ${peer.host}:${peer.port}`);
      this.onPeerFound(peer);
    });

    // Handle peer disappearance
    this.browser.on('down', (service: Service) => {
      const txt = service.txt as Record<string, string> | undefined;
      const deviceId = txt?.device_id || service.name;
      if (this.knownPeers.has(deviceId)) {
        this.knownPeers.delete(deviceId);
        console.log(`[PeerDiscovery] Peer lost: ${deviceId}`);
        this.onPeerLost(deviceId);
      }
    });

    console.log(`[PeerDiscovery] Browsing for mesh ${meshId.slice(0, 8)}...`);
  }

  stop(): void {
    if (this.browser) {
      this.browser.stop();
      this.browser = null;
    }
    this.bonjour.destroy();
    this.published = false;
    this.knownPeers.clear();
    console.log('[PeerDiscovery] Stopped');
  }
}
