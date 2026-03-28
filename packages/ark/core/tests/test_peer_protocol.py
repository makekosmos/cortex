"""Unit tests for the P2P peer protocol module."""

from core.peer_protocol import (
    PeerChange,
    PeerHello,
    PeerHelloAck,
    compute_auth_hmac,
    compute_mesh_id,
    generate_nonce,
    make_message,
    parse_message,
    verify_auth_hmac,
)


def test_compute_mesh_id():
    """mesh_id is deterministic for the same secret."""
    secret = "my-secret-key"
    id1 = compute_mesh_id(secret)
    id2 = compute_mesh_id(secret)
    assert id1 == id2
    assert len(id1) == 16
    # Different secret -> different mesh_id
    assert compute_mesh_id("other-secret") != id1


def test_hmac_verify_valid():
    """Correct secret and nonce produces a valid HMAC."""
    secret = "test-secret"
    nonce = "abc123"
    mac = compute_auth_hmac(secret, nonce)
    assert verify_auth_hmac(secret, nonce, mac) is True


def test_hmac_verify_invalid():
    """Wrong secret fails verification."""
    nonce = "abc123"
    mac = compute_auth_hmac("correct-secret", nonce)
    assert verify_auth_hmac("wrong-secret", nonce, mac) is False
    # Tampered HMAC also fails
    assert verify_auth_hmac("correct-secret", nonce, "deadbeef") is False


def test_peer_hello_create():
    """PeerHello.create populates nonce, HMAC, and mesh_id."""
    hello = PeerHello.create(
        device_id="dev-1",
        device_name="My Mac",
        platform="macos",
        mesh_secret="secret123",
    )
    assert hello.protocol_version == 2
    assert hello.device_id == "dev-1"
    assert hello.device_name == "My Mac"
    assert hello.platform == "macos"
    assert hello.mesh_id == compute_mesh_id("secret123")
    assert len(hello.nonce) == 64  # 32 bytes hex
    assert verify_auth_hmac("secret123", hello.nonce, hello.auth_hmac) is True


def test_peer_hello_ack_create():
    """PeerHelloAck.create populates nonce and HMAC."""
    ack = PeerHelloAck.create(
        ok=True,
        device_id="srv-1",
        device_name="Server",
        platform="linux",
        mesh_secret="secret123",
    )
    assert ack.ok is True
    assert ack.error is None
    assert verify_auth_hmac("secret123", ack.nonce, ack.auth_hmac) is True

    # Error variant
    ack_err = PeerHelloAck.create(
        ok=False,
        device_id="srv-1",
        device_name="Server",
        platform="linux",
        mesh_secret="secret123",
        error="auth_failed",
    )
    assert ack_err.ok is False
    assert ack_err.error == "auth_failed"


def test_peer_change_serialization():
    """PeerChange roundtrips through to_dict/from_dict."""
    original = PeerChange(
        event_id="evt-001",
        change_type="create",
        data={"event_type": "task", "summary": "Buy milk"},
        origin_device="mac-abc",
        origin_seq=42,
        hlc="2026-03-28T14:30:00.000000Z:000001:mac-abc",
        hop_path=["mac-abc", "relay-1"],
    )
    d = original.to_dict()
    restored = PeerChange.from_dict(d)
    assert restored.event_id == original.event_id
    assert restored.change_type == original.change_type
    assert restored.data == original.data
    assert restored.origin_device == original.origin_device
    assert restored.origin_seq == original.origin_seq
    assert restored.hlc == original.hlc
    assert restored.hop_path == original.hop_path


def test_nonce_uniqueness():
    """Two generate_nonce() calls produce different values."""
    n1 = generate_nonce()
    n2 = generate_nonce()
    assert n1 != n2
    assert len(n1) == 64
    assert len(n2) == 64


def test_make_and_parse_message():
    """make_message and parse_message are inverse operations."""
    raw = make_message("peer_hello", device_id="d1", nonce="abc")
    parsed = parse_message(raw)
    assert parsed["type"] == "peer_hello"
    assert parsed["device_id"] == "d1"
    assert parsed["nonce"] == "abc"
