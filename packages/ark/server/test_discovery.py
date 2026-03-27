"""Simple test: start broadcaster + discoverer, verify peer is found."""

import time

from server.discovery import ArkServiceBroadcaster, ArkServiceDiscoverer


def test_discovery() -> None:
    found_peers: list[dict] = []

    def on_found(peer):
        found_peers.append({"host": peer.host, "port": peer.port, "name": peer.name})
        print(f"  Found peer: {peer.name} at {peer.host}:{peer.port}")

    def on_removed(name):
        print(f"  Peer removed: {name}")

    broadcaster = ArkServiceBroadcaster(port=8765, device_name="test-device")
    discoverer = ArkServiceDiscoverer(on_found=on_found, on_removed=on_removed)

    print("Starting broadcaster...")
    broadcaster.start()

    print("Starting discoverer...")
    discoverer.start()

    # Give mDNS time to propagate on loopback
    print("Waiting for discovery (up to 5s)...")
    for _ in range(50):
        if found_peers:
            break
        time.sleep(0.1)

    peers = discoverer.get_peers()
    print(f"Discovered peers: {peers}")

    assert len(peers) >= 1, f"Expected at least 1 peer, got {len(peers)}"
    assert peers[0]["name"] == "test-device"
    assert peers[0]["port"] == 8765
    print("PASS: service discovered successfully")

    print("Stopping...")
    discoverer.stop()
    broadcaster.stop()
    print("Done.")


if __name__ == "__main__":
    test_discovery()
