"""
Device pairing system for Ark.

Generates short-lived pairing codes that new devices use to obtain
server credentials (URL + API key) without manual configuration.
Codes are single-use and expire after 5 minutes.
"""

from __future__ import annotations

import base64
import io
import os
import secrets
import string
import threading
import time
import uuid
from datetime import datetime, timezone
from typing import Any, Optional

# ---------------------------------------------------------------------------
# Pairing code store (in-memory, single-process)
# ---------------------------------------------------------------------------

_CODE_TTL = 300  # 5 minutes

_codes: dict[str, dict[str, Any]] = {}
_lock = threading.Lock()


def _cleanup_expired() -> None:
    """Remove expired codes. Must be called under _lock."""
    now = time.time()
    expired = [code for code, info in _codes.items() if info["expires_at"] < now]
    for code in expired:
        del _codes[code]


def generate_pairing_code() -> str:
    """Create a short 8-char alphanumeric code like ``ark-7f3k``."""
    charset = string.ascii_lowercase + string.digits
    suffix = "".join(secrets.choice(charset) for _ in range(4))
    return f"ark-{suffix}"


def generate_pairing_payload(
    server_url: str,
    api_key: str,
    device_name: str,
    code: str,
) -> dict[str, Any]:
    """Return the JSON payload that a scanning device receives."""
    return {
        "server_url": server_url,
        "api_key": api_key,
        "device_name": device_name,
        "created_at": datetime.now(timezone.utc).strftime("%Y-%m-%dT%H:%M:%SZ"),
        "code": code,
    }


def generate_qr_data_url(payload: dict[str, Any]) -> str:
    """Render *payload* as a QR-code PNG and return a ``data:`` URL."""
    import json
    import qrcode  # type: ignore[import-untyped]

    img = qrcode.make(json.dumps(payload, ensure_ascii=False))
    buf = io.BytesIO()
    img.save(buf, format="PNG")
    b64 = base64.b64encode(buf.getvalue()).decode()
    return f"data:image/png;base64,{b64}"


def create_pairing(server_url: str, api_key: str, device_name: str) -> dict[str, Any]:
    """
    Generate a new pairing code, store it, and return the public response.

    Returns dict with keys: code, qr_data_url, expires_in, payload.
    """
    code = generate_pairing_code()
    payload = generate_pairing_payload(server_url, api_key, device_name, code)
    qr_data_url = generate_qr_data_url(payload)

    with _lock:
        _cleanup_expired()
        _codes[code] = {
            "payload": payload,
            "expires_at": time.time() + _CODE_TTL,
        }

    return {
        "code": code,
        "qr_data_url": qr_data_url,
        "expires_in": _CODE_TTL,
    }


def claim_pairing(code: str) -> Optional[dict[str, Any]]:
    """
    Claim a pairing code. Returns the payload if valid, else None.

    The code is deleted after a successful claim (single-use).
    """
    with _lock:
        _cleanup_expired()
        entry = _codes.pop(code, None)

    if entry is None:
        return None
    if entry["expires_at"] < time.time():
        return None
    return entry["payload"]


def generate_device_id() -> str:
    """Generate a unique device ID for the claiming device."""
    return str(uuid.uuid4())[:12]
