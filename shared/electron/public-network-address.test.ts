import { expect, test } from "../test-support/node-test.mjs";
import { isPublicNetworkAddress } from "./public-network-address";

test("isPublicNetworkAddress rejects local and reserved addresses", () => {
  for (const address of [
    "127.0.0.1",
    "10.0.0.1",
    "169.254.169.254",
    "172.16.0.1",
    "192.168.1.1",
    "::1",
    "::127.0.0.1",
    "fc00::1",
    "fe80::1",
    // IPv4-mapped IPv6 spellings equivalent to private/loopback addresses.
    "::ffff:c0a8:1",
    "0:0:0:0:0:ffff:c0a8:1",
    "0:0:0:0:0:ffff:a9fe:a9fe",
    "0::ffff:7f00:1",
    // Same literal with a non-zero first hextet is still reserved space.
    "0:1:2:3:4:5:6:7",
    // Documentation range in non-canonical spelling.
    "2001:0db8::1",
  ]) {
    expect(isPublicNetworkAddress(address)).toBe(false);
  }
  expect(isPublicNetworkAddress("8.8.8.8")).toBe(true);
  expect(isPublicNetworkAddress("2606:4700:4700::1111")).toBe(true);
  // Expanded IPv4-mapped spellings of a public address stay public.
  expect(isPublicNetworkAddress("::ffff:8.8.8.8")).toBe(true);
  expect(isPublicNetworkAddress("0:0:0:0:0:ffff:808:808")).toBe(true);
});
