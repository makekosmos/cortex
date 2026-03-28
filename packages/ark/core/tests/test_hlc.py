"""Tests for HLC (Hybrid Logical Clock)."""

import time
from datetime import datetime, timezone
from unittest.mock import patch

from core.hlc import HLC


def _make_wall(ts: str) -> str:
    """Helper: return a fixed ISO wall-clock string."""
    return ts


def test_tick_advances_time():
    """tick() produces a later HLC."""
    hlc = HLC.now("dev-a")
    ticked = hlc.tick()
    assert ticked > hlc or ticked == hlc  # at minimum same wall + higher counter
    # More specifically: the ticked version must not be less
    assert not (ticked < HLC.from_str(str(hlc)))


def test_tick_increments_counter_same_time():
    """When wall clock hasn't advanced, counter increments."""
    fixed_wall = "2026-03-28T12:00:00.000000Z"
    hlc = HLC(wall_time=fixed_wall, counter=0, device_id="dev-a")

    with patch("core.hlc.datetime") as mock_dt:
        mock_dt.now.return_value = datetime(2026, 3, 28, 12, 0, 0, 0, tzinfo=timezone.utc)
        mock_dt.side_effect = lambda *a, **kw: datetime(*a, **kw)
        ticked = hlc.tick()

    assert ticked.wall_time == fixed_wall
    assert ticked.counter == 1


def test_tick_resets_counter_on_time_advance():
    """When wall clock advances, counter resets to 0."""
    old_wall = "2026-03-28T10:00:00.000000Z"
    hlc = HLC(wall_time=old_wall, counter=5, device_id="dev-a")

    # Real time is ahead of old_wall, so tick() should advance
    ticked = hlc.tick()
    assert ticked.wall_time > old_wall
    assert ticked.counter == 0


def test_merge_remote_ahead():
    """Remote has higher wall_time, merged clock adopts it."""
    local = HLC(wall_time="2026-03-28T10:00:00.000000Z", counter=3, device_id="dev-a")
    remote = HLC(wall_time="2026-03-28T12:00:00.000000Z", counter=5, device_id="dev-b")

    with patch("core.hlc.datetime") as mock_dt:
        mock_dt.now.return_value = datetime(2026, 3, 28, 11, 0, 0, 0, tzinfo=timezone.utc)
        mock_dt.side_effect = lambda *a, **kw: datetime(*a, **kw)
        merged = local.merge(remote)

    assert merged.wall_time == remote.wall_time
    assert merged.counter == remote.counter + 1
    assert merged.device_id == "dev-a"  # keeps local device_id


def test_merge_remote_behind():
    """Remote has lower wall_time, local wins."""
    local = HLC(wall_time="2026-03-28T12:00:00.000000Z", counter=3, device_id="dev-a")
    remote = HLC(wall_time="2026-03-28T10:00:00.000000Z", counter=5, device_id="dev-b")
    original_counter = local.counter

    with patch("core.hlc.datetime") as mock_dt:
        mock_dt.now.return_value = datetime(2026, 3, 28, 11, 0, 0, 0, tzinfo=timezone.utc)
        mock_dt.side_effect = lambda *a, **kw: datetime(*a, **kw)
        merged = local.merge(remote)

    assert merged.wall_time == "2026-03-28T12:00:00.000000Z"
    assert merged.counter == original_counter + 1  # 3 + 1 = 4


def test_merge_same_time_different_counter():
    """Same wall_time, takes max counter + 1."""
    same_wall = "2026-03-28T12:00:00.000000Z"
    local = HLC(wall_time=same_wall, counter=3, device_id="dev-a")
    remote = HLC(wall_time=same_wall, counter=7, device_id="dev-b")

    with patch("core.hlc.datetime") as mock_dt:
        mock_dt.now.return_value = datetime(2026, 3, 28, 11, 0, 0, 0, tzinfo=timezone.utc)
        mock_dt.side_effect = lambda *a, **kw: datetime(*a, **kw)
        merged = local.merge(remote)

    assert merged.wall_time == same_wall
    assert merged.counter == 8  # max(3, 7) + 1


def test_serialization_roundtrip():
    """str() -> from_str() preserves all fields."""
    original = HLC(
        wall_time="2026-03-28T14:30:00.123456Z",
        counter=42,
        device_id="mac-a1b2c3d4",
    )
    serialized = str(original)
    assert serialized == "2026-03-28T14:30:00.123456Z:000042:mac-a1b2c3d4"

    restored = HLC.from_str(serialized)
    assert restored.wall_time == original.wall_time
    assert restored.counter == original.counter
    assert restored.device_id == original.device_id
    assert restored == original


def test_comparison_operators():
    """<, >, ==, <=, >= work correctly."""
    a = HLC(wall_time="2026-03-28T10:00:00.000000Z", counter=0, device_id="dev-a")
    b = HLC(wall_time="2026-03-28T12:00:00.000000Z", counter=0, device_id="dev-a")

    assert a < b
    assert b > a
    assert not (a > b)
    assert not (b < a)
    assert a <= b
    assert b >= a
    assert a != b

    # Same values
    c = HLC(wall_time="2026-03-28T10:00:00.000000Z", counter=0, device_id="dev-a")
    assert a == c
    assert a <= c
    assert a >= c

    # Counter comparison
    d = HLC(wall_time="2026-03-28T10:00:00.000000Z", counter=1, device_id="dev-a")
    assert a < d
    assert d > a


def test_comparison_tiebreaker_device_id():
    """Same time+counter, device_id breaks tie."""
    a = HLC(wall_time="2026-03-28T10:00:00.000000Z", counter=0, device_id="dev-a")
    b = HLC(wall_time="2026-03-28T10:00:00.000000Z", counter=0, device_id="dev-b")

    assert a < b  # "dev-a" < "dev-b" lexicographically
    assert b > a
    assert a != b


def test_now_creates_valid_hlc():
    """HLC.now() produces parseable ISO timestamp."""
    hlc = HLC.now("test-device")
    assert hlc.device_id == "test-device"
    assert hlc.counter == 0

    # Verify wall_time is valid ISO format ending with Z
    assert hlc.wall_time.endswith("Z")
    # Should parse without error
    datetime.strptime(hlc.wall_time, "%Y-%m-%dT%H:%M:%S.%fZ")

    # Roundtrip
    restored = HLC.from_str(str(hlc))
    assert restored == hlc
