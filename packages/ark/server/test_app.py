"""
Tests for the Ark FastAPI server (server/app.py).

Run:
    cd packages/ark
    .venv/bin/python -m pytest server/test_app.py -v
"""

from __future__ import annotations

import os
import shutil
import tempfile
from pathlib import Path
from typing import Any
from unittest.mock import patch

import pytest

# ---------------------------------------------------------------------------
# Configure env BEFORE importing app (module-level side effects)
# ---------------------------------------------------------------------------

_temp_dir = tempfile.mkdtemp()
_db_path = str(Path(_temp_dir) / "test_app.db")

os.environ["LIFE_DB_PATH"] = _db_path
os.environ["LIFE_API_KEY"] = "test-secret-key"
os.environ["ARK_MDNS"] = "0"  # disable mDNS in tests

from fastapi.testclient import TestClient  # noqa: E402

from server.app import API_KEY, app, db  # noqa: E402

client = TestClient(app)
HEADERS = {"X-API-Key": API_KEY}


# ---------------------------------------------------------------------------
# Fixtures
# ---------------------------------------------------------------------------


@pytest.fixture(autouse=True)
def _clean_db() -> Any:
    """Wipe events between tests so they stay independent."""
    yield
    with db.connection() as conn:
        conn.execute("DELETE FROM events")
        conn.execute("DELETE FROM event_entity_links")
        conn.execute("DELETE FROM entities")


# ---------------------------------------------------------------------------
# Health / docs
# ---------------------------------------------------------------------------


class TestHealth:
    def test_health_no_auth(self) -> None:
        """Health endpoint does not require auth."""
        r = client.get("/health")
        assert r.status_code == 200
        assert r.json()["ok"] is True

    def test_openapi_docs(self) -> None:
        """OpenAPI spec should be served."""
        r = client.get("/openapi.json")
        assert r.status_code == 200
        assert "paths" in r.json()


# ---------------------------------------------------------------------------
# Auth
# ---------------------------------------------------------------------------


class TestAuth:
    def test_missing_api_key(self) -> None:
        r = client.get("/events")
        assert r.status_code == 401

    def test_wrong_api_key(self) -> None:
        r = client.get("/events", headers={"X-API-Key": "wrong"})
        assert r.status_code == 401

    def test_bearer_token(self) -> None:
        r = client.get(
            "/events",
            headers={"Authorization": f"Bearer {API_KEY}"},
        )
        assert r.status_code == 200

    def test_x_api_key_header(self) -> None:
        r = client.get("/events", headers=HEADERS)
        assert r.status_code == 200


# ---------------------------------------------------------------------------
# POST /events  (create)
# ---------------------------------------------------------------------------


class TestCreateEvent:
    def test_create_minimal(self) -> None:
        r = client.post(
            "/events",
            json={"event_type": "note", "data": {"text": "hello"}},
            headers=HEADERS,
        )
        assert r.status_code == 200
        body = r.json()
        assert "id" in body
        assert len(body["id"]) == 36

    def test_create_full(self) -> None:
        r = client.post(
            "/events",
            json={
                "event_type": "heart_rate",
                "data": {"bpm": 72},
                "category": "health",
                "occurred_at": "2024-06-01T12:00:00Z",
                "summary": "Resting HR",
                "duration_seconds": 30,
                "timezone": "UTC",
                "source": "garmin",
                "source_id": "g-001",
                "device": "watch",
                "tags": ["morning"],
            },
            headers=HEADERS,
        )
        assert r.status_code == 200
        event_id = r.json()["id"]

        # verify round-trip
        r2 = client.get(f"/events/{event_id}", headers=HEADERS)
        assert r2.status_code == 200
        ev = r2.json()
        assert ev["event_type"] == "heart_rate"
        assert ev["category"] == "health"
        assert ev["source"] == "garmin"
        assert ev["tags"] == ["morning"]

    def test_create_upsert_by_source_id(self) -> None:
        payload = {
            "event_type": "note",
            "data": {"v": 1},
            "source": "test",
            "source_id": "dup-1",
        }
        r1 = client.post("/events", json=payload, headers=HEADERS)
        assert r1.status_code == 200
        id1 = r1.json()["id"]

        payload["data"] = {"v": 2}
        r2 = client.post("/events", json=payload, headers=HEADERS)
        assert r2.status_code == 200
        id2 = r2.json()["id"]

        assert id1 == id2  # upsert reuses same id


# ---------------------------------------------------------------------------
# GET /events  (list / query)
# ---------------------------------------------------------------------------


class TestListEvents:
    def _seed(self) -> list[str]:
        ids = []
        for i in range(3):
            r = client.post(
                "/events",
                json={
                    "event_type": "note",
                    "data": {"i": i},
                    "category": "test",
                    "summary": f"note {i}",
                },
                headers=HEADERS,
            )
            ids.append(r.json()["id"])
        return ids

    def test_list_all(self) -> None:
        self._seed()
        r = client.get("/events", headers=HEADERS)
        assert r.status_code == 200
        assert len(r.json()) == 3

    def test_filter_by_event_type(self) -> None:
        self._seed()
        client.post(
            "/events",
            json={"event_type": "meal", "data": {}},
            headers=HEADERS,
        )
        r = client.get("/events?event_type=note", headers=HEADERS)
        assert len(r.json()) == 3

    def test_filter_by_category(self) -> None:
        self._seed()
        r = client.get("/events?category=test", headers=HEADERS)
        assert len(r.json()) == 3
        r2 = client.get("/events?category=nonexistent", headers=HEADERS)
        assert len(r2.json()) == 0

    def test_limit_offset(self) -> None:
        self._seed()
        r = client.get("/events?limit=2&offset=0", headers=HEADERS)
        assert len(r.json()) == 2
        r2 = client.get("/events?limit=2&offset=2", headers=HEADERS)
        assert len(r2.json()) == 1


# ---------------------------------------------------------------------------
# GET /events/{id}
# ---------------------------------------------------------------------------


class TestGetEvent:
    def test_get_existing(self) -> None:
        r = client.post(
            "/events",
            json={"event_type": "note", "data": {"a": 1}},
            headers=HEADERS,
        )
        eid = r.json()["id"]
        r2 = client.get(f"/events/{eid}", headers=HEADERS)
        assert r2.status_code == 200
        assert r2.json()["id"] == eid

    def test_get_not_found(self) -> None:
        r = client.get("/events/nonexistent-id-12345", headers=HEADERS)
        assert r.status_code == 404


# ---------------------------------------------------------------------------
# DELETE /events/{id}
# ---------------------------------------------------------------------------


class TestDeleteEvent:
    def test_delete_existing(self) -> None:
        r = client.post(
            "/events",
            json={"event_type": "note", "data": {}},
            headers=HEADERS,
        )
        eid = r.json()["id"]

        r2 = client.delete(f"/events/{eid}", headers=HEADERS)
        assert r2.status_code == 200
        assert r2.json()["ok"] is True

        # now it's gone
        r3 = client.get(f"/events/{eid}", headers=HEADERS)
        assert r3.status_code == 404

    def test_delete_not_found(self) -> None:
        r = client.delete("/events/no-such-id", headers=HEADERS)
        assert r.status_code == 404


# ---------------------------------------------------------------------------
# GET /search
# ---------------------------------------------------------------------------


class TestSearch:
    def test_search_finds_match(self) -> None:
        client.post(
            "/events",
            json={"event_type": "note", "data": {}, "summary": "headache migraine"},
            headers=HEADERS,
        )
        client.post(
            "/events",
            json={"event_type": "note", "data": {}, "summary": "sunshine walk"},
            headers=HEADERS,
        )
        r = client.get("/search?query=headache", headers=HEADERS)
        assert r.status_code == 200
        results = r.json()
        assert len(results) == 1
        assert "headache" in results[0]["summary"].lower()

    def test_search_empty_query(self) -> None:
        r = client.get("/search?query=", headers=HEADERS)
        assert r.status_code == 200
        assert r.json() == []

    def test_search_no_results(self) -> None:
        r = client.get("/search?query=zzzznonexistent", headers=HEADERS)
        assert r.status_code == 200
        assert r.json() == []


# ---------------------------------------------------------------------------
# GET /db/stats
# ---------------------------------------------------------------------------


class TestStats:
    def test_stats_empty(self) -> None:
        r = client.get("/db/stats", headers=HEADERS)
        assert r.status_code == 200
        body = r.json()
        assert body["total_events"] == 0

    def test_stats_after_inserts(self) -> None:
        client.post(
            "/events",
            json={"event_type": "note", "data": {}, "category": "work"},
            headers=HEADERS,
        )
        client.post(
            "/events",
            json={"event_type": "meal", "data": {}, "category": "nutrition"},
            headers=HEADERS,
        )
        r = client.get("/db/stats", headers=HEADERS)
        body = r.json()
        assert body["total_events"] == 2
        cat_names = [c["category"] for c in body["categories"]]
        assert "work" in cat_names
        assert "nutrition" in cat_names

    def test_stats_raw(self) -> None:
        r = client.get("/stats/raw", headers=HEADERS)
        assert r.status_code == 200
        assert "events_count" in r.json()


# ---------------------------------------------------------------------------
# POST /events/page  (paging endpoint)
# ---------------------------------------------------------------------------


class TestPageEndpoint:
    def _seed(self, n: int = 5) -> None:
        for i in range(n):
            client.post(
                "/events",
                json={
                    "event_type": "note",
                    "data": {"i": i},
                    "category": "test",
                    "summary": f"page note {i}",
                },
                headers=HEADERS,
            )

    def test_basic_page(self) -> None:
        self._seed()
        r = client.post(
            "/events/page",
            json={"offset": 0, "limit": 3},
            headers=HEADERS,
        )
        assert r.status_code == 200
        body = r.json()
        assert body["total_count"] == 5
        assert len(body["rows"]) == 3
        assert body["has_more"] is True

    def test_page_with_filter(self) -> None:
        self._seed()
        client.post(
            "/events",
            json={"event_type": "meal", "data": {}, "category": "food"},
            headers=HEADERS,
        )
        r = client.post(
            "/events/page",
            json={
                "offset": 0,
                "limit": 100,
                "filters": [
                    {"column": "category", "operator": "Equals", "value": "test"},
                ],
            },
            headers=HEADERS,
        )
        body = r.json()
        assert body["total_count"] == 5


# ---------------------------------------------------------------------------
# POST /events/batch
# ---------------------------------------------------------------------------


class TestBatchEvents:
    def test_batch_create(self) -> None:
        r = client.post(
            "/events/batch",
            json=[
                {"event_type": "note", "data": {"n": 1}},
                {"event_type": "note", "data": {"n": 2}},
            ],
            headers=HEADERS,
        )
        assert r.status_code == 200
        body = r.json()
        assert body["created"] == 2

    def test_batch_empty(self) -> None:
        r = client.post("/events/batch", json=[], headers=HEADERS)
        assert r.status_code == 200
        body = r.json()
        assert body["created"] == 0


# ---------------------------------------------------------------------------
# Cleanup: remove temp dir at process exit (not at module teardown,
# because the shared `db` object may still be used by other test modules).
# ---------------------------------------------------------------------------

import atexit as _atexit

_atexit.register(shutil.rmtree, _temp_dir, True)
