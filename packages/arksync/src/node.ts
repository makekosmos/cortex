/**
 * Node.js-only utilities. Do NOT import in browser/renderer code.
 */

/**
 * Collect all non-loopback IP addresses from network interfaces.
 * Requires Node.js `os` module — only works in Node.js/Electron main process.
 */
export function getOwnAddresses(port: number): string[] {
  // eslint-disable-next-line @typescript-eslint/no-require-imports
  const os = require("node:os");
  const addresses: string[] = [];
  const interfaces = os.networkInterfaces();
  for (const [name, nets] of Object.entries(interfaces)) {
    for (const net of (nets as Array<{
      internal: boolean;
      family: string;
      address: string;
    }>) ?? []) {
      if (net.internal) continue;
      if (net.family === "IPv4") {
        addresses.push(`${net.address}:${port}`);
      } else if (net.family === "IPv6") {
        addresses.push(`[${net.address}%${name}]:${port}`);
      }
    }
  }
  return addresses;
}
