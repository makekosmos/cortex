/**
 * Node.js-only utilities. Do NOT import in browser/renderer code.
 */

/** Interface-name prefixes that belong to virtual / VPN / container adapters. */
const VIRTUAL_IFACE_PREFIXES = [
  // macOS
  "utun", // VPN / Tailscale / iCloud Private Relay
  "awdl", // AirDrop
  "llw", // Low-Latency WLAN (AWDL sibling)
  "ap", // AP mode
  "bridge", // Internet Sharing / iBridge
  "anpi", // Apple Neural Processor Interface
  // Linux
  "docker",
  "br-",
  "veth",
  "virbr",
  "vboxnet",
  "vmnet",
  "tun",
  "tap",
  "wg", // WireGuard
  "tailscale",
  // Windows
  "vEthernet",
  "VMware",
  "VirtualBox",
];

function isVirtualInterface(name: string): boolean {
  const lower = name.toLowerCase();
  return VIRTUAL_IFACE_PREFIXES.some((p) => lower.startsWith(p.toLowerCase()));
}

/** Check whether an IPv4 string is a link-local auto-assigned address (169.254/16). */
function isLinkLocalV4(addr: string): boolean {
  return addr.startsWith("169.254.");
}

/** Check whether an IPv6 string is link-local (fe80::/10). */
function isLinkLocalV6(addr: string): boolean {
  const lower = addr.toLowerCase();
  return (
    lower.startsWith("fe8") ||
    lower.startsWith("fe9") ||
    lower.startsWith("fea") ||
    lower.startsWith("feb")
  );
}

/** Check whether an IPv6 string is unique-local (fc00::/7 — unroutable outside a private site). */
function isUniqueLocalV6(addr: string): boolean {
  const lower = addr.toLowerCase();
  return lower.startsWith("fc") || lower.startsWith("fd");
}

/**
 * Collect all routable non-loopback IP addresses from network interfaces.
 *
 * Filters out (Syncthing-style):
 *   - loopback (`internal=true`)
 *   - IPv4 link-local (169.254/16)
 *   - IPv6 link-local (fe80::/10) and unique-local (fc00::/7)
 *   - virtual / VPN / container interfaces (utun, docker, vboxnet, wg, …)
 *
 * Unfiltered addresses are unreachable from peers and spam SyncClient with
 * "All addresses failed" logs.
 *
 * Requires Node.js `os` module — only works in Node.js/Electron main process.
 */
export function getOwnAddresses(port: number): string[] {
  // eslint-disable-next-line @typescript-eslint/no-require-imports
  const os = require("node:os");
  const addresses: string[] = [];
  const interfaces = os.networkInterfaces() as Record<
    string,
    Array<{ internal: boolean; family: string; address: string }> | undefined
  >;
  for (const [name, nets] of Object.entries(interfaces)) {
    if (isVirtualInterface(name)) continue;
    for (const net of nets ?? []) {
      if (net.internal) continue;
      if (net.family === "IPv4") {
        if (isLinkLocalV4(net.address)) continue;
        addresses.push(`${net.address}:${port}`);
      } else if (net.family === "IPv6") {
        if (isLinkLocalV6(net.address)) continue;
        if (isUniqueLocalV6(net.address)) continue;
        addresses.push(`[${net.address}]:${port}`);
      }
    }
  }
  return addresses;
}
