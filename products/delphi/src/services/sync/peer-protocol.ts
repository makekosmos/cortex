/**
 * Peer-to-peer protocol types and crypto helpers for LAN mesh sync.
 *
 * Standalone implementation (arksync package removed).
 */

import { createHmac, randomBytes, timingSafeEqual } from "crypto";

// ---------------------------------------------------------------------------
// Crypto helpers
// ---------------------------------------------------------------------------

export function computeMeshId(meshSecret: string): string {
  return createHmac("sha256", "mesh-id").update(meshSecret).digest("hex").slice(0, 16);
}

export function computeAuthHmac(meshSecret: string, nonce: string): string {
  return createHmac("sha256", meshSecret).update(nonce).digest("hex");
}

export function verifyAuthHmac(meshSecret: string, nonce: string, provided: string): boolean {
  const expected = computeAuthHmac(meshSecret, nonce);
  try {
    return timingSafeEqual(Buffer.from(expected, "hex"), Buffer.from(provided, "hex"));
  } catch {
    return false;
  }
}

export function generateNonce(): string {
  return randomBytes(32).toString("hex");
}

// ---------------------------------------------------------------------------
// Message types
// ---------------------------------------------------------------------------

export interface PeerHello {
  type: "peer_hello";
  protocol_version: 2;
  device_id: string;
  device_name: string;
  platform: string;
  mesh_id: string;
  nonce: string;
  auth_hmac: string;
}

export interface PeerHelloAck {
  type: "peer_hello_ack";
  ok: boolean;
  device_id: string;
  device_name: string;
  platform: string;
  nonce: string;
  auth_hmac: string;
  error?: string;
}

export interface PeerChange {
  type: "change";
  event_id: string;
  change_type: "create" | "update" | "delete";
  data: Record<string, unknown>;
  origin_device: string;
  origin_seq: number;
  hlc: string;
  hop_path: string[];
}

// ---------------------------------------------------------------------------
// Message constructors
// ---------------------------------------------------------------------------

export function createPeerHello(
  deviceId: string,
  deviceName: string,
  platform: string,
  meshSecret: string,
): PeerHello {
  const nonce = generateNonce();
  return {
    type: "peer_hello",
    protocol_version: 2,
    device_id: deviceId,
    device_name: deviceName,
    platform,
    mesh_id: computeMeshId(meshSecret),
    nonce,
    auth_hmac: computeAuthHmac(meshSecret, nonce),
  };
}

export function createPeerHelloAck(
  ok: boolean,
  deviceId: string,
  deviceName: string,
  platform: string,
  meshSecret: string,
  error?: string,
): PeerHelloAck {
  const nonce = generateNonce();
  const msg: PeerHelloAck = {
    type: "peer_hello_ack",
    ok,
    device_id: deviceId,
    device_name: deviceName,
    platform,
    nonce,
    auth_hmac: computeAuthHmac(meshSecret, nonce),
  };
  if (error) msg.error = error;
  return msg;
}
