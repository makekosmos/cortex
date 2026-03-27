#!/usr/bin/env python3
"""
Sync Elysium nutrition data to Ark.

Run from repo root:
    python3 -m plugins.sync_elysium --input /path/to/elysium.db [--output life.db]

Options:
    --input PATH    Path to Elysium SQLite database (required)
    --output PATH   Output Ark database path (default: elysium_ark.db in repo root)
    --full          Full sync (default: incremental from last synced entry)
    --days N        Sync last N days

Examples:
    # Full sync from Elysium export
    python3 -m plugins.sync_elysium --input ~/elysium.db --output ~/life.db --full

    # Incremental sync (last 7 days)
    python3 -m plugins.sync_elysium --input ~/elysium.db --output ~/life.db --days 7
"""

from __future__ import annotations

import sys
from pathlib import Path

from core.ark import Ark
from plugins.elysium import ElysiumPlugin


def main() -> None:
    project_root = Path(__file__).resolve().parent.parent

    # ── Parse arguments ──

    if "--help" in sys.argv or "-h" in sys.argv or len(sys.argv) < 2:
        print(__doc__.strip())
        sys.exit(0)

    # --input (required)
    input_path: Path | None = None
    if "--input" in sys.argv:
        idx = sys.argv.index("--input")
        if idx + 1 < len(sys.argv):
            input_path = Path(sys.argv[idx + 1])
            if not input_path.is_absolute():
                input_path = Path.cwd() / input_path
            input_path = input_path.resolve()

    if not input_path:
        print("Error: --input is required (path to Elysium SQLite database)")
        sys.exit(1)

    if not input_path.exists():
        print(f"Error: Elysium database not found: {input_path}")
        sys.exit(1)

    # --output
    output_path = project_root / "elysium_ark.db"
    if "--output" in sys.argv:
        idx = sys.argv.index("--output")
        if idx + 1 < len(sys.argv):
            output_path = Path(sys.argv[idx + 1])
            if not output_path.is_absolute():
                output_path = (project_root / output_path).resolve()
            else:
                output_path = output_path.resolve()

    full_sync = "--full" in sys.argv

    days: int | None = None
    if "--days" in sys.argv:
        idx = sys.argv.index("--days")
        if idx + 1 < len(sys.argv):
            days = int(sys.argv[idx + 1])

    # ── Run sync ──

    print(f"Input:  {input_path}")
    print(f"Output: {output_path}")

    if full_sync:
        print("Mode:   Full sync")
    elif days:
        print(f"Mode:   Last {days} days")
    else:
        print("Mode:   Incremental sync")

    ark = Ark(output_path)
    plugin = ElysiumPlugin(input_path)

    try:
        results = plugin.sync_to_ark(
            ark,
            days=days,
            full_sync=full_sync,
        )

        total_created = 0
        total_updated = 0
        total_skipped = 0

        print("\nСинхронизация завершена!")
        print()

        for event_type, (created, updated, skipped) in results.items():
            print(f"  {event_type}:")
            print(f"    Создано:   {created}")
            print(f"    Обновлено: {updated}")
            print(f"    Пропущено: {skipped}")
            total_created += created
            total_updated += updated
            total_skipped += skipped

        print()
        print(f"  Итого: +{total_created} / ~{total_updated} / ={total_skipped}")

        # Show DB stats
        stats = ark.get_stats()
        print("\nСтатистика БД:")
        print(f"  Всего событий:  {stats['events_count']}")
        print(f"  Всего сущностей: {stats['entities_count']}")

        if stats["event_types"]:
            print("  Типы событий:")
            for et, count in sorted(stats["event_types"].items()):
                print(f"    {et}: {count}")

    except FileNotFoundError as e:
        print(f"\nОшибка: {e}")
        sys.exit(1)
    except Exception as e:
        print(f"\nОшибка: {e}")
        sys.exit(1)


if __name__ == "__main__":
    main()
