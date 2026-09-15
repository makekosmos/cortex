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
  ]) {
    expect(isPublicNetworkAddress(address)).toBe(false);
  }
  expect(isPublicNetworkAddress("8.8.8.8")).toBe(true);
  expect(isPublicNetworkAddress("2606:4700:4700::1111")).toBe(true);
});
