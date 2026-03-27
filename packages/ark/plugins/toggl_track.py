"""
Toggl Track Plugin for Ark

Imports time entries from Toggl Track into Ark database.
Uses Reports API v3 for full data export with pagination.

Usage:
    from toggl_track import TogglTrackPlugin

    plugin = TogglTrackPlugin(api_token="your_api_token")
    plugin.sync_to_ark(db)  # Full sync
    plugin.sync_to_ark(db, days=30)  # Last 30 days
"""

from __future__ import annotations

import base64
import json
import sys
import time
import urllib.error
import urllib.request
from dataclasses import dataclass
from datetime import datetime, timedelta, timezone
from typing import Any, Callable, Optional, Union

from core.ark import Ark, parse_iso8601, to_iso8601

# ============================================================================
# Data Classes
# ============================================================================


@dataclass
class TogglTimeEntry:
    """Represents a Toggl time entry from Reports API."""

    id: int
    workspace_id: int
    project_id: Optional[int]
    task_id: Optional[int]
    billable: bool
    start: str
    stop: Optional[str]
    duration: int  # in seconds
    description: Optional[str]
    tags: list[str]
    user_id: int
    at: str  # last updated

    # Enriched fields
    project_name: Optional[str] = None
    client_name: Optional[str] = None
    username: Optional[str] = None

    @classmethod
    def from_reports_api_grouped(
        cls,
        group_data: dict[str, Any],
        entry_data: dict[str, Any],
        workspace_id: int = 0,
    ) -> TogglTimeEntry:
        """
        Create from Toggl Reports API v3 response.

        Reports API returns grouped data where each row contains metadata
        and a `time_entries` array with the actual entries.
        """
        return cls(
            id=entry_data["id"],
            workspace_id=workspace_id,
            project_id=group_data.get("project_id"),
            task_id=group_data.get("task_id"),
            billable=group_data.get("billable", False),
            start=entry_data["start"],
            stop=entry_data.get("stop"),
            duration=entry_data.get("seconds", 0),
            description=group_data.get("description") or None,
            tags=[],  # tag_ids provided, not tag names
            user_id=group_data.get("user_id", 0),
            at=entry_data.get("at", ""),
            # Enriched fields
            project_name=group_data.get("project"),
            client_name=group_data.get("client"),
            username=group_data.get("username"),
        )


@dataclass
class TogglProject:
    """Represents a Toggl project."""

    id: int
    workspace_id: int
    client_id: Optional[int]
    name: str
    is_private: bool
    active: bool
    color: str
    billable: Optional[bool]

    @classmethod
    def from_api(cls, data: dict[str, Any]) -> TogglProject:
        """Create from Toggl API response."""
        return cls(
            id=data["id"],
            workspace_id=data["workspace_id"],
            client_id=data.get("client_id"),
            name=data["name"],
            is_private=data.get("is_private", False),
            active=data.get("active", True),
            color=data.get("color", ""),
            billable=data.get("billable"),
        )


@dataclass
class TogglClient:
    """Represents a Toggl client."""

    id: int
    workspace_id: int
    name: str
    archived: bool

    @classmethod
    def from_api(cls, data: dict[str, Any]) -> TogglClient:
        """Create from Toggl API response."""
        return cls(
            id=data["id"],
            workspace_id=data["wid"],
            name=data["name"],
            archived=data.get("archived", False),
        )


# ============================================================================
# Toggl Track API Client
# ============================================================================


class TogglTrackAPI:
    """
    Toggl Track API client.

    Uses:
    - API v9 for workspaces, projects, clients
    - Reports API v3 for time entries (full export with pagination)
    """

    BASE_URL = "https://api.track.toggl.com/api/v9"
    REPORTS_URL = "https://api.track.toggl.com/reports/api/v3"

    # Rate limit: 1 request per second
    REQUEST_DELAY = 1.0

    def __init__(self, api_token: str):
        """
        Initialize API client.

        Args:
            api_token: Toggl Track API token
        """
        self.api_token = api_token
        self._auth_header = self._make_auth_header(api_token)
        self._projects_cache: dict[int, TogglProject] = {}
        self._clients_cache: dict[int, TogglClient] = {}
        self._last_request_time: float = 0

    def _make_auth_header(self, api_token: str) -> str:
        """Create Basic Auth header value."""
        credentials = f"{api_token}:api_token"
        encoded = base64.b64encode(credentials.encode()).decode()
        return f"Basic {encoded}"

    def _rate_limit(self) -> None:
        """Enforce rate limit between requests."""
        elapsed = time.time() - self._last_request_time
        if elapsed < self.REQUEST_DELAY:
            time.sleep(self.REQUEST_DELAY - elapsed)
        self._last_request_time = time.time()

    def _request(
        self,
        method: str,
        url: str,
        data: Optional[dict[str, Any]] = None,
    ) -> tuple[Any, dict[str, str]]:
        """
        Make API request with rate limiting.

        Returns:
            Tuple of (response_data, response_headers)
        """
        self._rate_limit()

        headers = {
            "Authorization": self._auth_header,
            "Content-Type": "application/json",
        }

        body = json.dumps(data).encode() if data else None

        request = urllib.request.Request(url, data=body, headers=headers, method=method)

        try:
            with urllib.request.urlopen(request, timeout=30) as response:
                response_headers = {k.lower(): v for k, v in response.getheaders()}
                response_data = json.loads(response.read().decode())
                return response_data, response_headers
        except urllib.error.HTTPError as e:
            error_body = e.read().decode() if e.fp else ""

            # Handle rate limit (HTTP 402 on Toggl)
            if e.code == 402 and "quota" in error_body.lower():
                # Extract wait time from error message
                wait_seconds = self._parse_quota_wait_time(error_body)
                raise TogglQuotaError(
                    f"Лимит API исчерпан. Ожидание: {wait_seconds} сек.",
                    wait_seconds=wait_seconds,
                ) from e

            raise TogglAPIError(f"HTTP {e.code}: {error_body}") from e
        except urllib.error.URLError as e:
            raise TogglAPIError(f"Network error: {e.reason}") from e

    def _parse_quota_wait_time(self, error_body: str) -> int:
        """Parse wait time from quota error message."""
        import re

        # Look for "reset in N seconds" pattern
        match = re.search(r"reset in (\d+) seconds", error_body, re.IGNORECASE)
        if match:
            return int(match.group(1))
        # Default to 1 hour if not found
        return 3600

    def _request_simple(self, method: str, endpoint: str) -> Any:
        """Simple request to base API (no pagination)."""
        url = f"{self.BASE_URL}{endpoint}"
        data, _ = self._request(method, url)
        return data

    def get_me(self) -> dict[str, Any]:
        """Get current user info."""
        return self._request_simple("GET", "/me")

    def get_workspaces(self) -> list[dict[str, Any]]:
        """Get user's workspaces."""
        return self._request_simple("GET", "/workspaces")

    def get_workspace_id(self) -> int:
        """Get the first workspace ID."""
        workspaces = self.get_workspaces()
        if not workspaces:
            raise TogglAPIError("No workspaces found")
        return workspaces[0]["id"]

    def get_projects(self, workspace_id: int) -> list[TogglProject]:
        """Get projects for workspace."""
        response = self._request_simple("GET", f"/workspaces/{workspace_id}/projects")

        if response is None:
            return []

        projects = [TogglProject.from_api(p) for p in response]

        # Cache projects
        for project in projects:
            self._projects_cache[project.id] = project

        return projects

    def get_clients(self, workspace_id: int) -> list[TogglClient]:
        """Get clients for workspace."""
        response = self._request_simple("GET", f"/workspaces/{workspace_id}/clients")

        if response is None:
            return []

        clients = [TogglClient.from_api(c) for c in response]

        # Cache clients
        for client in clients:
            self._clients_cache[client.id] = client

        return clients

    def get_project(self, project_id: int) -> Optional[TogglProject]:
        """Get project by ID (uses cache)."""
        return self._projects_cache.get(project_id)

    def get_client(self, client_id: int) -> Optional["TogglClient"]:
        """Get client by ID (uses cache)."""
        return self._clients_cache.get(client_id)

    def load_workspace_data(self, workspace_id: int) -> None:
        """Load all projects and clients for workspace."""
        self.get_projects(workspace_id)
        self.get_clients(workspace_id)

    def _get_entries_for_period(
        self,
        workspace_id: int,
        start_date: str,
        end_date: str,
        page_size: int = 50,
        on_progress: Optional[Callable[[int, int], None]] = None,
        total_so_far: int = 0,
    ) -> list[TogglTimeEntry]:
        """
        Get time entries for a single period (max 366 days).

        Returns:
            List of time entries for the period
        """
        url = f"{self.REPORTS_URL}/workspace/{workspace_id}/search/time_entries"

        period_entries: list[TogglTimeEntry] = []
        first_row_number: Optional[int] = None
        page_num = 0

        while True:
            page_num += 1

            # Build request body
            body: dict[str, Any] = {
                "start_date": start_date,
                "end_date": end_date,
                "page_size": page_size,
                "order_by": "date",
                "order_dir": "ASC",
            }

            if first_row_number is not None:
                body["first_row_number"] = first_row_number

            # Make request
            response_data, headers = self._request("POST", url, body)

            if response_data is None:
                break

            # Parse entries - Reports API returns grouped data
            for group in response_data:
                time_entries = group.get("time_entries", [])
                for entry in time_entries:
                    period_entries.append(
                        TogglTimeEntry.from_reports_api_grouped(
                            group_data=group,
                            entry_data=entry,
                            workspace_id=workspace_id,
                        )
                    )

            # Progress callback
            if on_progress:
                on_progress(total_so_far + len(period_entries), page_num)

            # Check for next page
            next_row = headers.get("x-next-row-number")
            if next_row:
                first_row_number = int(next_row)
            else:
                # No more pages
                break

        return period_entries

    def get_time_entries_paginated(
        self,
        workspace_id: int,
        start_date: str,
        end_date: str,
        page_size: int = 50,
        on_progress: Optional[Callable[[int, int], None]] = None,
    ) -> list[TogglTimeEntry]:
        """
        Get all time entries using Reports API v3 with pagination.

        Automatically splits requests into 365-day chunks to comply with API limits.

        Args:
            workspace_id: Workspace ID
            start_date: Start date (YYYY-MM-DD)
            end_date: End date (YYYY-MM-DD)
            page_size: Number of entries per page (default 50)
            on_progress: Optional callback(entries_count, page_num)

        Returns:
            List of all time entries
        """
        # Parse dates
        start_dt = datetime.strptime(start_date, "%Y-%m-%d")
        end_dt = datetime.strptime(end_date, "%Y-%m-%d")

        # Max 365 days per request (API limit is 366, use 365 to be safe)
        max_days = 365

        all_entries: list[TogglTimeEntry] = []
        current_start = start_dt

        while current_start <= end_dt:
            # Calculate end of this chunk
            current_end = min(current_start + timedelta(days=max_days), end_dt)

            # Fetch entries for this period
            period_entries = self._get_entries_for_period(
                workspace_id=workspace_id,
                start_date=current_start.strftime("%Y-%m-%d"),
                end_date=current_end.strftime("%Y-%m-%d"),
                page_size=page_size,
                on_progress=on_progress,
                total_so_far=len(all_entries),
            )

            all_entries.extend(period_entries)

            # Move to next chunk
            current_start = current_end + timedelta(days=1)

        return all_entries


class TogglAPIError(Exception):
    """Toggl API error."""

    pass


class TogglQuotaError(TogglAPIError):
    """Toggl API quota exceeded error."""

    def __init__(self, message: str, wait_seconds: int = 3600):
        super().__init__(message)
        self.wait_seconds = wait_seconds


# ============================================================================
# Toggl Track Plugin
# ============================================================================


class TogglTrackPlugin:
    """
    Plugin to import Toggl Track data into Ark.

    Maps Toggl time entries to events with:
    - event_type: "time_entry"
    - category: "productivity"
    - source: "toggl_track"

    Features:
    - Full data export using Reports API v3
    - Pagination for large datasets
    - Deduplication via source_id
    - Incremental sync (only fetch new entries)
    """

    EVENT_TYPE = "time_entry"
    CATEGORY = "productivity"
    SOURCE = "toggl_track"

    def __init__(self, api_token: str):
        """
        Initialize plugin.

        Args:
            api_token: Toggl Track API token
        """
        self.api = TogglTrackAPI(api_token)

    def _convert_entry_to_event(
        self,
        entry: TogglTimeEntry,
    ) -> dict[str, Any]:
        """Convert Toggl time entry to Ark event dict."""
        # Build data payload
        data: dict[str, Any] = {
            "toggl_id": entry.id,
            "workspace_id": entry.workspace_id,
            "billable": entry.billable,
            "user_id": entry.user_id,
        }

        if entry.description:
            data["description"] = entry.description

        if entry.project_id:
            data["project_id"] = entry.project_id

        # Use enriched data from Reports API
        if entry.project_name:
            data["project_name"] = entry.project_name

        if entry.client_name:
            data["client_name"] = entry.client_name

        if entry.task_id:
            data["task_id"] = entry.task_id

        # Build summary
        parts = []
        if entry.description:
            parts.append(entry.description)
        if entry.project_name:
            parts.append(f"[{entry.project_name}]")
        if entry.client_name:
            parts.append(f"({entry.client_name})")

        summary = " ".join(parts) if parts else "Time entry"

        # Calculate duration in seconds
        duration_seconds = entry.duration if entry.duration > 0 else None

        return {
            "event_type": self.EVENT_TYPE,
            "category": self.CATEGORY,
            "occurred_at": entry.start,
            "data": data,
            "summary": summary,
            "duration_seconds": duration_seconds,
            "source": self.SOURCE,
            "source_id": str(entry.id),
            "tags": entry.tags,
        }

    def _get_last_sync_date(self, db: Ark) -> Optional[str]:
        """Get the date of the most recent synced entry."""
        events = db.query_events(
            source=self.SOURCE,
            limit=1,
            order="DESC",
        )
        if events:
            # Return date part only (YYYY-MM-DD)
            return events[0].occurred_at[:10]
        return None

    def fetch_entries(
        self,
        start_date: Optional[Union[datetime, str]] = None,
        end_date: Optional[Union[datetime, str]] = None,
        on_progress: Optional[Callable[[int, int], None]] = None,
    ) -> list[TogglTimeEntry]:
        """
        Fetch time entries from Toggl using Reports API v3.

        Args:
            start_date: Start of date range (default: 2010-01-01 for full export)
            end_date: End of date range (default: today)
            on_progress: Optional callback(entries_count, page_num)

        Returns:
            List of time entries
        """
        # Get workspace ID
        workspace_id = self.api.get_workspace_id()

        # Format dates
        if start_date is None:
            start_str = "2010-01-01"
        elif isinstance(start_date, datetime):
            start_str = start_date.strftime("%Y-%m-%d")
        else:
            start_str = start_date

        if end_date is None:
            end_str = datetime.now(timezone.utc).strftime("%Y-%m-%d")
        elif isinstance(end_date, datetime):
            end_str = end_date.strftime("%Y-%m-%d")
        else:
            end_str = end_date

        # Load workspace data for project/client enrichment
        self.api.load_workspace_data(workspace_id)

        return self.api.get_time_entries_paginated(
            workspace_id=workspace_id,
            start_date=start_str,
            end_date=end_str,
            on_progress=on_progress,
        )

    def sync_to_ark(
        self,
        db: Ark,
        days: Optional[int] = None,
        start_date: Optional[Union[datetime, str]] = None,
        end_date: Optional[Union[datetime, str]] = None,
        full_sync: bool = False,
        on_progress: Optional[Callable[[int, int], None]] = None,
    ) -> tuple[int, int, int]:
        """
        Sync Toggl time entries to Ark.

        Idempotency: Uses (source, source_id) (Toggl entry ID) to create or update existing entries.

        Args:
            db: Ark database instance
            days: Number of days to fetch (overrides start_date)
            start_date: Start of date range
            end_date: End of date range
            full_sync: If True, fetch all data from 2010
            on_progress: Optional callback(entries_count, page_num)

        Returns:
            Tuple of (created, updated, skipped) counts
        """
        # Determine date range
        if full_sync:
            start_date = "2010-01-01"
        elif days is not None:
            start_date = datetime.now(timezone.utc) - timedelta(days=days)
        elif start_date is None:
            # Incremental sync: start from last synced entry
            last_date = self._get_last_sync_date(db)
            if last_date:
                # Go back 1 day to catch any late entries
                dt = datetime.strptime(last_date, "%Y-%m-%d")
                start_date = (dt - timedelta(days=1)).strftime("%Y-%m-%d")
            else:
                # No previous data, do full sync
                start_date = "2010-01-01"

        # Get existing IDs for fast deduplication
        # Fetch entries from Toggl
        entries = self.fetch_entries(start_date, end_date, on_progress)

        if not entries:
            return (0, 0, 0)

        # Convert to events
        events = [self._convert_entry_to_event(e) for e in entries]

        # Calculate date range for import tracking
        occurred_dts = [parse_iso8601(e["occurred_at"]) for e in events]
        date_range_start = to_iso8601(min(occurred_dts))
        date_range_end = to_iso8601(max(occurred_dts))

        # Start import tracking
        import_id = db.start_import(
            source=self.SOURCE,
            file_name=f"api_sync_{datetime.now(timezone.utc).strftime('%Y%m%d_%H%M%S')}",
        )

        try:
            # Batch import (record_events_batch also does deduplication, but we pre-filter)
            created, updated, batch_skipped = db.record_events_batch(
                events,
                source=self.SOURCE,
            )

            # Complete import
            db.complete_import(
                import_id,
                records_created=created,
                records_updated=updated,
                records_skipped=batch_skipped,
                date_range_start=date_range_start,
                date_range_end=date_range_end,
            )

            return (created, updated, batch_skipped)

        except Exception as e:
            db.fail_import(import_id, str(e))
            raise

    def create_project_entities(self, db: Ark) -> int:
        """
        Create Ark entities for Toggl projects.

        Args:
            db: Ark database instance

        Returns:
            Number of entities created
        """
        created = 0

        for project in self.api._projects_cache.values():
            # Check if entity already exists
            existing = db.find_entity(
                entity_type="project",
                name=project.name,
            )

            if existing:
                continue

            data = {
                "toggl_id": project.id,
                "workspace_id": project.workspace_id,
                "color": project.color,
                "active": project.active,
                "billable": project.billable,
            }

            if project.client_id:
                client = self.api.get_client(project.client_id)
                if client:
                    data["client_id"] = project.client_id
                    data["client_name"] = client.name

            db.create_entity(
                entity_type="project",
                name=project.name,
                data=data,
            )
            created += 1

        return created

    def create_client_entities(self, db: Ark) -> int:
        """
        Create Ark entities for Toggl clients.

        Args:
            db: Ark database instance

        Returns:
            Number of entities created
        """
        created = 0

        for client in self.api._clients_cache.values():
            # Check if entity already exists
            existing = db.find_entity(
                entity_type="client",
                name=client.name,
            )

            if existing:
                continue

            db.create_entity(
                entity_type="client",
                name=client.name,
                data={
                    "toggl_id": client.id,
                    "workspace_id": client.workspace_id,
                    "archived": client.archived,
                },
            )
            created += 1

        return created


# ============================================================================
# CLI Interface
# ============================================================================


def main() -> None:
    """CLI interface for Toggl Track plugin."""
    import os

    # Get API token from environment or argument
    api_token = os.environ.get("TOGGL_TRACK")

    if len(sys.argv) < 2:
        print("Usage: python toggl_track.py <database.db> [--full | --days N]")
        print()
        print("Options:")
        print("  --full     Full sync from 2010")
        print("  --days N   Sync last N days (default: incremental)")
        print()
        print("Environment variables:")
        print("  TOGGL_TRACK - Toggl Track API token")
        return

    db_path = sys.argv[1]
    full_sync = "--full" in sys.argv
    days = None

    # Parse --days argument
    if "--days" in sys.argv:
        days_idx = sys.argv.index("--days")
        if days_idx + 1 < len(sys.argv):
            days = int(sys.argv[days_idx + 1])

    if not api_token:
        print("Error: TOGGL_TRACK environment variable not set")
        return

    print(f"Syncing Toggl Track data to {db_path}...")
    if full_sync:
        print("Mode: Full sync (from 2010)")
    elif days:
        print(f"Mode: Last {days} days")
    else:
        print("Mode: Incremental sync")

    db = Ark(db_path)
    plugin = TogglTrackPlugin(api_token)

    def on_progress(count: int, page: int) -> None:
        print(f"  Fetched {count} entries (page {page})...", end="\r")

    try:
        created, updated, skipped = plugin.sync_to_ark(
            db,
            days=days,
            full_sync=full_sync,
            on_progress=on_progress,
        )

        print()  # Clear progress line
        print("Sync complete!")
        print(f"  Created: {created}")
        print(f"  Updated: {updated}")
        print(f"  Skipped: {skipped}")

        # Create entities
        projects_created = plugin.create_project_entities(db)
        clients_created = plugin.create_client_entities(db)

        if projects_created or clients_created:
            print("Entities created:")
            print(f"  Projects: {projects_created}")
            print(f"  Clients: {clients_created}")

        # Show stats
        stats = db.get_stats()
        print("\nDatabase stats:")
        print(f"  Total events: {stats['events_count']}")
        print(f"  Total entities: {stats['entities_count']}")

    except TogglAPIError as e:
        print(f"\nToggl API error: {e}")
        sys.exit(1)


if __name__ == "__main__":
    main()
