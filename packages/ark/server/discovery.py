"""mDNS service discovery for Ark sync on LAN.

Uses zeroconf to broadcast/discover Ark server instances on the local network.
This enables direct P2P sync between devices on the same WiFi, bypassing VPS.
"""

from __future__ import annotations

import logging
import socket
import threading
from dataclasses import dataclass, field
from typing import Callable, Optional

from zeroconf import ServiceBrowser, ServiceInfo, ServiceStateChange, Zeroconf

logger = logging.getLogger(__name__)

SERVICE_TYPE = "_ark-sync._tcp.local."
API_VERSION = "1"


def _get_local_ip() -> str:
    """Best-effort LAN IP address of this machine.

    Prefers physical/WiFi interfaces (en0, eth0) over VPN tunnels (utun*, tun*, wg*).
    Falls back to the UDP-trick if no suitable interface is found.
    """
    import fcntl
    import struct

    _PREFERRED = ("en0", "eth0", "wlan0", "en1", "en2")
    _VPN_PREFIXES = ("utun", "tun", "wg", "vpn", "ppp")
    SIOCGIFADDR = 0x8915  # Linux; macOS uses same ioctl number

    def _iface_ip(name: str) -> Optional[str]:
        try:
            s = socket.socket(socket.AF_INET, socket.SOCK_DGRAM)
            try:
                result = fcntl.ioctl(
                    s.fileno(),
                    SIOCGIFADDR,
                    struct.pack("256s", name[:15].encode()),
                )
                ip = socket.inet_ntoa(result[20:24])
                return ip if not ip.startswith("127.") else None
            finally:
                s.close()
        except Exception:
            return None

    # 1. Try preferred physical interfaces first
    for name in _PREFERRED:
        ip = _iface_ip(name)
        if ip:
            return ip

    # 2. Any non-loopback, non-VPN interface via /proc/net/if_inet6 or getifaddrs
    try:
        import subprocess

        out = subprocess.check_output(
            ["ifconfig"], stderr=subprocess.DEVNULL, text=True
        )
        current_iface = ""
        for line in out.splitlines():
            if line and not line[0].isspace():
                current_iface = line.split(":")[0].split()[0]
            if "inet " in line and current_iface:
                if any(current_iface.startswith(p) for p in _VPN_PREFIXES):
                    continue
                if current_iface.startswith("lo"):
                    continue
                ip = line.strip().split()[1]
                if not ip.startswith("127."):
                    return ip
    except Exception:
        pass

    # 3. Fallback: UDP trick (may pick VPN)
    try:
        s = socket.socket(socket.AF_INET, socket.SOCK_DGRAM)
        try:
            s.connect(("8.8.8.8", 80))
            return s.getsockname()[0]
        finally:
            s.close()
    except Exception:
        return "127.0.0.1"


@dataclass
class ArkPeer:
    host: str
    port: int
    name: str
    api_version: str = API_VERSION


class ArkServiceBroadcaster:
    """Broadcasts this Ark server on the LAN via mDNS."""

    def __init__(self, port: int, device_name: str) -> None:
        self.port = port
        self.device_name = device_name
        self._zeroconf: Optional[Zeroconf] = None
        self._info: Optional[ServiceInfo] = None

    def start(self) -> None:
        local_ip = _get_local_ip()
        self._info = ServiceInfo(
            SERVICE_TYPE,
            name=f"{self.device_name}.{SERVICE_TYPE}",
            addresses=[socket.inet_aton(local_ip)],
            port=self.port,
            properties={
                "device_name": self.device_name,
                "port": str(self.port),
                "api_version": API_VERSION,
                "pairing_url": f"http://{local_ip}:{self.port}/pairing/qr",
            },
        )
        self._zeroconf = Zeroconf()
        self._zeroconf.register_service(self._info)
        logger.info(
            "mDNS broadcasting on %s:%d as '%s'", local_ip, self.port, self.device_name
        )

    def stop(self) -> None:
        if self._zeroconf and self._info:
            self._zeroconf.unregister_service(self._info)
            self._zeroconf.close()
            self._zeroconf = None
            self._info = None
            logger.info("mDNS broadcast stopped")


class ArkServiceDiscoverer:
    """Listens for Ark services on the LAN via mDNS."""

    def __init__(
        self,
        on_found: Optional[Callable[[ArkPeer], None]] = None,
        on_removed: Optional[Callable[[str], None]] = None,
    ) -> None:
        self.on_found = on_found
        self.on_removed = on_removed
        self._peers: dict[str, ArkPeer] = {}
        self._lock = threading.Lock()
        self._zeroconf: Optional[Zeroconf] = None
        self._browser: Optional[ServiceBrowser] = None

    def start(self) -> None:
        self._zeroconf = Zeroconf()
        self._browser = ServiceBrowser(
            self._zeroconf, SERVICE_TYPE, handlers=[self._on_state_change]
        )
        logger.info("mDNS discovery started, listening for Ark peers")

    def stop(self) -> None:
        if self._browser:
            self._browser.cancel()
            self._browser = None
        if self._zeroconf:
            self._zeroconf.close()
            self._zeroconf = None
        with self._lock:
            self._peers.clear()
        logger.info("mDNS discovery stopped")

    def get_peers(self) -> list[dict[str, object]]:
        with self._lock:
            return [
                {"host": p.host, "port": p.port, "name": p.name}
                for p in self._peers.values()
            ]

    def _on_state_change(
        self,
        zeroconf: Zeroconf,
        service_type: str,
        name: str,
        state_change: ServiceStateChange,
    ) -> None:
        if state_change == ServiceStateChange.Added:
            info = zeroconf.get_service_info(service_type, name)
            if info is None:
                return
            addresses = info.parsed_addresses()
            if not addresses:
                return
            host = addresses[0]
            port = info.port
            if port is None:
                return
            props = info.properties or {}
            device_name = (props.get(b"device_name") or b"unknown").decode()
            api_version = (props.get(b"api_version") or b"1").decode()

            peer = ArkPeer(
                host=host, port=port, name=device_name, api_version=api_version
            )
            with self._lock:
                self._peers[name] = peer
            logger.info("Discovered Ark peer: %s at %s:%d", device_name, host, port)
            if self.on_found:
                self.on_found(peer)

        elif state_change == ServiceStateChange.Removed:
            with self._lock:
                removed = self._peers.pop(name, None)
            if removed:
                logger.info("Ark peer removed: %s", removed.name)
                if self.on_removed:
                    self.on_removed(name)
