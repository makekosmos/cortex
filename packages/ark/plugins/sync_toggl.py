#!/usr/bin/env python3
"""
Sync Toggl Track data to a database.

Run from repo root:
    python3 -m plugins.sync_toggl [--full] [--output PATH]

Options:
    --full          Full sync from 2010 (default: incremental)
    --output PATH   Output database path (default: toggl_test.db in repo root)

Reads TOGGL_API_TOKEN (or legacy TOGGL_TRACK) from .env.local in project root.
"""

from __future__ import annotations

import os
import sys
from pathlib import Path

from core.ark import Ark
from plugins.toggl_track import TogglAPIError, TogglQuotaError, TogglTrackPlugin


def load_env_file(env_file: Path) -> None:
    if not env_file.exists():
        return
    for line in env_file.read_text(encoding="utf-8").splitlines():
        line = line.strip()
        if line and not line.startswith("#") and "=" in line:
            key, value = line.split("=", 1)
            os.environ.setdefault(key, value)


def main() -> None:
    project_root = Path(__file__).resolve().parent.parent
    load_env_file(project_root / ".env.local")

    api_token = os.environ.get("TOGGL_API_TOKEN") or os.environ.get("TOGGL_TRACK")
    if not api_token:
        print("Error: TOGGL_API_TOKEN (or TOGGL_TRACK) not found in .env.local")
        sys.exit(1)

    full_sync = "--full" in sys.argv

    # Parse --output parameter
    db_path = project_root / "toggl_test.db"
    if "--output" in sys.argv:
        idx = sys.argv.index("--output")
        if idx + 1 < len(sys.argv):
            db_path = Path(sys.argv[idx + 1])
            if not db_path.is_absolute():
                db_path = (project_root / db_path).resolve()
            else:
                db_path = db_path.resolve()

    print(f"Database: {db_path}")
    db = Ark(db_path)

    print("Initializing Toggl Track plugin...")
    plugin = TogglTrackPlugin(api_token)

    print("Mode:", "Full sync (from 2010)" if full_sync else "Incremental sync")

    def on_progress(count: int, page: int) -> None:
        print(f"  Fetched {count} entries (page {page})...", end="\r")

    max_retries = 5
    retry_count = 0

    while retry_count < max_retries:
        try:
            created, updated, skipped = plugin.sync_to_ark(
                db,
                full_sync=full_sync,
                on_progress=on_progress,
            )

            print()  # Clear progress line
            print("\nСинхронизация завершена!")
            print(f"  Создано: {created}")
            print(f"  Обновлено: {updated}")
            print(f"  Пропущено: {skipped}")

            projects = plugin.create_project_entities(db)
            clients = plugin.create_client_entities(db)

            print("\nСущности созданы:")
            print(f"  Проекты: {projects}")
            print(f"  Клиенты: {clients}")

            stats = db.get_stats()
            print("\nСтатистика БД:")
            print(f"  Всего событий: {stats['events_count']}")
            print(f"  Всего сущностей: {stats['entities_count']}")

            if stats["event_types"]:
                print("  Типы событий:")
                for et, count in stats["event_types"].items():
                    print(f"    {et}: {count}")

            break

        except TogglQuotaError as e:
            retry_count += 1
            wait_time = e.wait_seconds

            print(
                f"\nЛимит API исчерпан. Ожидание {wait_time} сек (~{wait_time // 60} мин)..."
            )
            print(f"Попытка {retry_count}/{max_retries}")

            if retry_count >= max_retries:
                print("\nПревышено максимальное количество попыток.")
                sys.exit(1)

            import time

            time.sleep(wait_time + 5)
            print("\nПовторная попытка...")

        except TogglAPIError as e:
            print(f"\nОшибка Toggl API: {e}")
            sys.exit(1)


if __name__ == "__main__":
    main()
