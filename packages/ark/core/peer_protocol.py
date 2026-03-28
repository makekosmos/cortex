"""P2P peer protocol message definitions and authentication."""

import hashlib
import hmac
import json
import secrets
from dataclasses import dataclass, asdict
from typing import Optional


def compute_mesh_id(mesh_secret: str) -> str:
    """Derive mesh_id from shared secret: first 16 hex chars of SHA-256."""
    return hashlib.sha256(mesh_secret.encode()).hexdigest()[:16]


def compute_auth_hmac(mesh_secret: str, nonce: str) -> str:
    """HMAC-SHA256(mesh_secret, nonce) for mutual authentication."""
    return hmac.new(mesh_secret.encode(), nonce.encode(), hashlib.sha256).hexdigest()


def verify_auth_hmac(mesh_secret: str, nonce: str, provided_hmac: str) -> bool:
    """Constant-time HMAC verification."""
    expected = compute_auth_hmac(mesh_secret, nonce)
    return hmac.compare_digest(expected, provided_hmac)


def generate_nonce() -> str:
    return secrets.token_hex(32)


# ---------------------------------------------------------------------------
# Message types
# ---------------------------------------------------------------------------


@dataclass
class PeerHello:
    protocol_version: int  # = 2
    device_id: str
    device_name: str
    platform: str
    mesh_id: str
    nonce: str
    auth_hmac: str

    def to_dict(self) -> dict:
        return asdict(self)

    @classmethod
    def create(
        cls,
        device_id: str,
        device_name: str,
        platform: str,
        mesh_secret: str,
    ) -> "PeerHello":
        nonce = generate_nonce()
        mesh_id = compute_mesh_id(mesh_secret)
        auth = compute_auth_hmac(mesh_secret, nonce)
        return cls(
            protocol_version=2,
            device_id=device_id,
            device_name=device_name,
            platform=platform,
            mesh_id=mesh_id,
            nonce=nonce,
            auth_hmac=auth,
        )


@dataclass
class PeerHelloAck:
    ok: bool
    device_id: str
    device_name: str
    platform: str
    nonce: str
    auth_hmac: str
    error: Optional[str] = None

    def to_dict(self) -> dict:
        return asdict(self)

    @classmethod
    def create(
        cls,
        ok: bool,
        device_id: str,
        device_name: str,
        platform: str,
        mesh_secret: str,
        error: Optional[str] = None,
    ) -> "PeerHelloAck":
        nonce = generate_nonce()
        auth = compute_auth_hmac(mesh_secret, nonce)
        return cls(
            ok=ok,
            device_id=device_id,
            device_name=device_name,
            platform=platform,
            nonce=nonce,
            auth_hmac=auth,
            error=error,
        )


@dataclass
class PeerChange:
    """A change with P2P metadata for loop prevention."""

    event_id: str
    change_type: str  # create, update, delete
    data: dict
    origin_device: str
    origin_seq: int
    hlc: str
    hop_path: list[str]

    def to_dict(self) -> dict:
        return asdict(self)

    @classmethod
    def from_dict(cls, d: dict) -> "PeerChange":
        return cls(
            event_id=d["event_id"],
            change_type=d["change_type"],
            data=d["data"],
            origin_device=d["origin_device"],
            origin_seq=d["origin_seq"],
            hlc=d["hlc"],
            hop_path=d.get("hop_path", []),
        )


# ---------------------------------------------------------------------------
# Wire helpers
# ---------------------------------------------------------------------------


def parse_message(raw: str) -> dict:
    """Parse a JSON message and return the dict."""
    return json.loads(raw)


def make_message(msg_type: str, **kwargs) -> str:
    """Create a JSON message string."""
    return json.dumps({"type": msg_type, **kwargs})
