"""Hybrid Logical Clock for P2P sync conflict resolution."""

import threading
from datetime import datetime, timezone


class HLC:
    """
    Hybrid Logical Clock combining wall-clock time with a logical counter.

    Format: "<ISO8601_wall_clock>:<counter:06d>:<device_id>"
    Example: "2026-03-28T14:30:00.123456Z:000042:mac-a1b2c3d4"

    Comparison is lexicographic: timestamp first, then counter, then device_id as tiebreaker.
    """

    _lock = threading.Lock()

    def __init__(self, wall_time: str, counter: int, device_id: str):
        self.wall_time = wall_time
        self.counter = counter
        self.device_id = device_id

    @classmethod
    def now(cls, device_id: str) -> "HLC":
        wall = datetime.now(timezone.utc).strftime("%Y-%m-%dT%H:%M:%S.%fZ")
        return cls(wall_time=wall, counter=0, device_id=device_id)

    def tick(self) -> "HLC":
        """Advance the clock for a local event."""
        with self._lock:
            now_wall = datetime.now(timezone.utc).strftime("%Y-%m-%dT%H:%M:%S.%fZ")
            if now_wall > self.wall_time:
                new = HLC(wall_time=now_wall, counter=0, device_id=self.device_id)
            else:
                new = HLC(
                    wall_time=self.wall_time,
                    counter=self.counter + 1,
                    device_id=self.device_id,
                )
            self.wall_time = new.wall_time
            self.counter = new.counter
            return new

    def merge(self, remote: "HLC") -> "HLC":
        """Merge with a remote HLC (on receiving a change from another peer)."""
        with self._lock:
            now_wall = datetime.now(timezone.utc).strftime("%Y-%m-%dT%H:%M:%S.%fZ")
            max_wall = max(self.wall_time, remote.wall_time, now_wall)

            if max_wall == self.wall_time == remote.wall_time:
                new_counter = max(self.counter, remote.counter) + 1
            elif max_wall == self.wall_time:
                new_counter = self.counter + 1
            elif max_wall == remote.wall_time:
                new_counter = remote.counter + 1
            else:
                # now_wall is strictly the largest
                new_counter = 0

            new = HLC(
                wall_time=max_wall,
                counter=new_counter,
                device_id=self.device_id,
            )
            self.wall_time = new.wall_time
            self.counter = new.counter
            return new

    def _key(self):
        return (self.wall_time, self.counter, self.device_id)

    def __str__(self) -> str:
        return f"{self.wall_time}:{self.counter:06d}:{self.device_id}"

    @classmethod
    def from_str(cls, s: str) -> "HLC":
        # Split into exactly 3 parts: wall_time, counter, device_id
        # wall_time contains colons (ISO8601), so we split from the right
        parts = s.rsplit(":", 2)
        if len(parts) != 3:
            raise ValueError(f"Invalid HLC string: {s}")
        wall_time, counter_str, device_id = parts
        return cls(wall_time=wall_time, counter=int(counter_str), device_id=device_id)

    def __lt__(self, other):
        return self._key() < other._key()

    def __gt__(self, other):
        return self._key() > other._key()

    def __eq__(self, other):
        return self._key() == other._key()

    def __le__(self, other):
        return self._key() <= other._key()

    def __ge__(self, other):
        return self._key() >= other._key()
