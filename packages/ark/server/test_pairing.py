"""
Tests for the Ark pairing flow (server/pairing.py + pairing endpoints in app.py).

Run:
    cd packages/ark
    .venv/bin/python -m pytest server/test_pairing.py -v
"""

from __future__ import annotations

import os
import shutil
import tempfile
import time
from pathlib import Path
from typing import Any
from unittest.mock import patch

import pytest

# ---------------------------------------------------------------------------
# Configure env BEFORE importing app
# ---------------------------------------------------------------------------

_temp_dir = tempfile.mkdtemp()
_db_path = str(Path(_temp_dir) / "test_pairing.db")

os.environ["LIFE_DB_PATH"] = _db_path
os.environ.setdefault("LIFE_API_KEY", "test-pairing-key")
os.environ["ARK_MDNS"] = "0"

from fastapi.testclient import TestClient  # noqa: E402

from server.app import API_KEY, app, db  # noqa: E402
from server.pairing import (  # noqa: E402
    _codes,
    _lock,
    claim_pairing,
    create_pairing,
    generate_device_id,
    generate_pairing_code,
)

client = TestClient(app)
HEADERS = {"X-API-Key": API_KEY}


@pytest.fixture(autouse=True)
def _clean_pairing_codes() -> Any:
    """Clear the in-memory pairing store between tests."""
    yield
    with _lock:
        _codes.clear()


@pytest.fixture(autouse=True)
def _mock_qr() -> Any:
    """Mock QR generation to avoid Pillow dependency in tests."""
    with patch(
        "server.pairing.generate_qr_data_url",
        return_value="data:image/png;base64,FAKE_QR_DATA",
    ):
        yield


# ===========================================================================
# Unit tests: pairing.py functions
# ===========================================================================


class TestPairingCodeGeneration:
    def test_code_format(self) -> None:
        code = generate_pairing_code()
        assert code.startswith("ark-")
        assert len(code) == 8  # "ark-" + 4 chars

    def test_codes_unique(self) -> None:
        codes = {generate_pairing_code() for _ in range(200)}
        # with 36^4 space collisions are extremely unlikely in 200 samples
        assert len(codes) == 200

    def test_device_id_format(self) -> None:
        did = generate_device_id()
        assert len(did) == 12
        # should be a uuid prefix with hyphens
        assert isinstance(did, str)


class TestCreateAndClaimPairing:
    def test_create_returns_expected_keys(self) -> None:
        result = create_pairing("http://1.2.3.4:8000", "secret", "my-mac")
        assert "code" in result
        assert "qr_data_url" in result
        assert "expires_in" in result
        assert result["code"].startswith("ark-")
        assert result["qr_data_url"].startswith("data:image/png;base64,")
        assert result["expires_in"] == 300

    def test_claim_valid_code(self) -> None:
        result = create_pairing("http://1.2.3.4:8000", "secret", "my-mac")
        code = result["code"]

        payload = claim_pairing(code)
        assert payload is not None
        assert payload["server_url"] == "http://1.2.3.4:8000"
        assert payload["api_key"] == "secret"
        assert payload["device_name"] == "my-mac"

    def test_claim_single_use(self) -> None:
        result = create_pairing("http://1.2.3.4:8000", "secret", "my-mac")
        code = result["code"]

        # first claim succeeds
        assert claim_pairing(code) is not None
        # second claim fails (code consumed)
        assert claim_pairing(code) is None

    def test_claim_invalid_code(self) -> None:
        assert claim_pairing("ark-zzzz") is None

    def test_claim_expired_code(self) -> None:
        result = create_pairing("http://1.2.3.4:8000", "secret", "mac")
        code = result["code"]

        # Manually expire the code
        with _lock:
            _codes[code]["expires_at"] = time.time() - 1

        assert claim_pairing(code) is None


# ===========================================================================
# Integration tests: HTTP endpoints
# ===========================================================================


class TestPairingCreateEndpoint:
    @patch("server.discovery._get_local_ip", return_value="192.168.1.42")
    def test_create_pairing(self, _mock_ip: Any) -> None:
        r = client.post("/pairing/create", headers=HEADERS)
        assert r.status_code == 200
        body = r.json()
        assert body["code"].startswith("ark-")
        assert body["expires_in"] == 300
        assert "qr_data_url" in body

    def test_create_pairing_requires_auth(self) -> None:
        r = client.post("/pairing/create")
        assert r.status_code == 401


class TestPairingClaimEndpoint:
    @patch("server.discovery._get_local_ip", return_value="192.168.1.42")
    def test_claim_flow(self, _mock_ip: Any) -> None:
        """Full flow: create code via API, then claim it."""
        # Step 1: create
        r1 = client.post("/pairing/create", headers=HEADERS)
        assert r1.status_code == 200
        code = r1.json()["code"]

        # Step 2: claim (no auth required)
        r2 = client.post(
            "/pairing/claim",
            json={
                "code": code,
                "device_name": "Pixel 8",
                "platform": "android",
            },
        )
        assert r2.status_code == 200
        body = r2.json()
        assert "server_url" in body
        assert "api_key" in body
        assert "device_id" in body
        assert body["api_key"] == API_KEY
        assert len(body["device_id"]) == 12

    def test_claim_invalid_code(self) -> None:
        r = client.post(
            "/pairing/claim",
            json={"code": "ark-0000", "device_name": "Test"},
        )
        assert r.status_code == 404

    @patch("server.discovery._get_local_ip", return_value="192.168.1.42")
    def test_claim_twice_fails(self, _mock_ip: Any) -> None:
        r1 = client.post("/pairing/create", headers=HEADERS)
        code = r1.json()["code"]

        # first claim OK
        r2 = client.post(
            "/pairing/claim",
            json={"code": code, "device_name": "Phone"},
        )
        assert r2.status_code == 200

        # second claim fails
        r3 = client.post(
            "/pairing/claim",
            json={"code": code, "device_name": "Tablet"},
        )
        assert r3.status_code == 404

    @patch("server.discovery._get_local_ip", return_value="192.168.1.42")
    def test_claimed_device_registered(self, _mock_ip: Any) -> None:
        """After claiming, device should appear in sync_devices table."""
        r1 = client.post("/pairing/create", headers=HEADERS)
        code = r1.json()["code"]

        r2 = client.post(
            "/pairing/claim",
            json={"code": code, "device_name": "TestDevice", "platform": "ios"},
        )
        device_id = r2.json()["device_id"]

        with db.connection() as conn:
            row = conn.execute(
                "SELECT * FROM sync_devices WHERE device_id = ?",
                (device_id,),
            ).fetchone()
            assert row is not None
            assert row["name"] == "TestDevice"
            assert row["platform"] == "ios"


class TestPairingQREndpoint:
    @patch("server.discovery._get_local_ip", return_value="192.168.1.42")
    def test_qr_page_returns_html(self, _mock_ip: Any) -> None:
        r = client.get("/pairing/qr")
        assert r.status_code == 200
        assert "text/html" in r.headers["content-type"]
        assert "ark-" in r.text
        assert "Ark Pairing" in r.text

    def test_qr_no_auth_required(self) -> None:
        """QR page should be accessible without API key (opened from server machine)."""
        with patch("server.discovery._get_local_ip", return_value="10.0.0.1"):
            r = client.get("/pairing/qr")
        assert r.status_code == 200


# ---------------------------------------------------------------------------
# Cleanup at process exit (shared db may outlive any single test module).
# ---------------------------------------------------------------------------

import atexit as _atexit

_atexit.register(shutil.rmtree, _temp_dir, True)
