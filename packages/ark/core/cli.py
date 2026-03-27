"""
Ark CLI: Command-line interface for the Ark Life Database.

Usage:
    python3 -m core.cli <command> [options]

Examples:
    ark add "Купить кефир" --type task --category productivity --tags todo,shopping
    ark task "Написать тесты"
    ark list --category health --limit 10
    ark search "кефир"
    ark stats
"""

from __future__ import annotations

import argparse
import json
import os
import sys
from datetime import datetime, timezone
from pathlib import Path
from typing import Optional

from core.ark import Ark, parse_iso8601

# ============================================================================
# Constants
# ============================================================================

DEFAULT_DB_PATH = Path(os.environ.get("LIFE_DB_PATH", "./ark.db"))
DEFAULT_LIMIT = 20
SEARCH_HIGHLIGHT = "\033[1;33m"  # bold yellow
RESET = "\033[0m"
DIM = "\033[2m"
BOLD = "\033[1m"
GREEN = "\033[32m"
CYAN = "\033[36m"

# Detect if output supports colors
NO_COLOR = os.environ.get("NO_COLOR") or not sys.stdout.isatty()


def _c(code: str, text: str) -> str:
    """Apply ANSI color code, respecting NO_COLOR."""
    if NO_COLOR:
        return text
    return f"{code}{text}{RESET}"


# ============================================================================
# Formatting helpers
# ============================================================================


def _format_timestamp(iso_str: str) -> str:
    """Convert UTC ISO timestamp to local time, compact format."""
    try:
        dt = parse_iso8601(iso_str)
        local_dt = dt.astimezone(tz=None)  # system local timezone
        now = datetime.now(tz=None).astimezone(tz=None)
        if local_dt.date() == now.date():
            return local_dt.strftime("сегодня %H:%M")
        elif local_dt.year == now.year:
            return local_dt.strftime("%d %b %H:%M")
        else:
            return local_dt.strftime("%d %b %Y %H:%M")
    except (ValueError, OSError):
        return iso_str[:16]


def _truncate(text: str, max_len: int) -> str:
    """Truncate text with ellipsis."""
    if len(text) <= max_len:
        return text
    return text[: max_len - 1] + "\u2026"


def _format_size(size_bytes: int) -> str:
    """Format file size in human-readable form."""
    if size_bytes < 1024:
        return f"{size_bytes} B"
    elif size_bytes < 1024 * 1024:
        return f"{size_bytes / 1024:.1f} KB"
    else:
        return f"{size_bytes / 1024 / 1024:.2f} MB"


def _get_summary(event) -> str:
    """Extract a meaningful summary from an event."""
    if event.summary:
        return event.summary
    # Try to build summary from data
    data = event.data or {}
    if not data:
        return ""
    # Pick first meaningful value
    for key in ("description", "title", "name", "text", "note", "value"):
        if key in data:
            val = data[key]
            if isinstance(val, str):
                return val
            return str(val)
    # Fallback: compact JSON of first few fields
    items = list(data.items())[:3]
    parts = [f"{k}={v}" for k, v in items]
    return ", ".join(parts)


# ============================================================================
# Commands
# ============================================================================


def cmd_add(db: Ark, args: argparse.Namespace) -> None:
    """Add a new event."""
    tags = []
    if args.tags:
        tags = [t.strip() for t in args.tags.split(",") if t.strip()]

    data = {}
    if args.data:
        try:
            data = json.loads(args.data)
        except json.JSONDecodeError:
            print(f"Ошибка: невалидный JSON в --data: {args.data}", file=sys.stderr)
            sys.exit(1)

    event_id = db.record_event(
        event_type=args.type,
        data=data,
        category=args.category,
        summary=args.summary,
        tags=tags,
        source="cli",
    )
    print(f"{_c(GREEN, '+')} {args.summary or args.type}")
    print(f"  id: {_c(DIM, event_id[:8])}")


def cmd_task(db: Ark, args: argparse.Namespace) -> None:
    """Quick shortcut: add a task."""
    event_id = db.record_event(
        event_type="task",
        data={},
        category="productivity",
        summary=args.summary,
        tags=_parse_tags(args),
        source="cli",
    )
    print(f"{_c(GREEN, '+')} task: {args.summary}")
    print(f"  id: {_c(DIM, event_id[:8])}")


def cmd_note(db: Ark, args: argparse.Namespace) -> None:
    """Quick shortcut: add a note."""
    event_id = db.record_event(
        event_type="note",
        data={},
        category="productivity",
        summary=args.summary,
        tags=_parse_tags(args),
        source="cli",
    )
    print(f"{_c(GREEN, '+')} note: {args.summary}")
    print(f"  id: {_c(DIM, event_id[:8])}")


def cmd_health(db: Ark, args: argparse.Namespace) -> None:
    """Quick shortcut: add a health event."""
    event_type = args.type or "health_note"
    event_id = db.record_event(
        event_type=event_type,
        data={},
        category="health",
        summary=args.summary,
        tags=_parse_tags(args),
        source="cli",
    )
    print(f"{_c(GREEN, '+')} {event_type}: {args.summary}")
    print(f"  id: {_c(DIM, event_id[:8])}")


def _parse_tags(args: argparse.Namespace) -> list[str]:
    """Parse comma-separated tags from args."""
    if hasattr(args, "tags") and args.tags:
        return [t.strip() for t in args.tags.split(",") if t.strip()]
    return []


def cmd_list(db: Ark, args: argparse.Namespace) -> None:
    """List events with filters."""
    kwargs: dict = {
        "limit": args.limit,
        "order": "DESC",
    }
    if args.category:
        kwargs["category"] = args.category
    if args.type:
        kwargs["event_type"] = args.type
    if args.since:
        kwargs["start_date"] = args.since
    if hasattr(args, "source") and args.source:
        kwargs["source"] = args.source
    if hasattr(args, "tags") and args.tags:
        kwargs["tags"] = [t.strip() for t in args.tags.split(",") if t.strip()]

    events = db.query_events(**kwargs)

    if not events:
        print(_c(DIM, "Нет событий."))
        return

    # Calculate column widths
    date_width = 16
    type_width = max(len(e.event_type) for e in events)
    type_width = min(max(type_width, 8), 20)

    # Header
    print(
        _c(DIM, f"{'Дата':<{date_width}}  {'Тип':<{type_width}}  Описание")
    )
    print(_c(DIM, "\u2500" * 70))

    for event in events:
        date_str = _format_timestamp(event.occurred_at)
        etype = _truncate(event.event_type, type_width)
        summary = _get_summary(event)
        summary = _truncate(summary, 45)

        cat_badge = ""
        if event.category:
            cat_badge = _c(DIM, f" [{event.category}]")

        print(
            f"{_c(CYAN, f'{date_str:<{date_width}}')}  "
            f"{etype:<{type_width}}  "
            f"{summary}{cat_badge}"
        )

    print(_c(DIM, f"\n  Показано {len(events)} событий"))


def cmd_search(db: Ark, args: argparse.Namespace) -> None:
    """Full-text search."""
    query = args.query
    limit = args.limit if hasattr(args, "limit") else 20

    events = db.search(query, limit=limit)

    if not events:
        print(_c(DIM, f'Ничего не найдено по запросу "{query}"'))
        return

    print(_c(BOLD, f'Результаты по "{query}":'))
    print()

    for event in events:
        date_str = _format_timestamp(event.occurred_at)
        summary = _get_summary(event)

        # Highlight matching text
        if not NO_COLOR:
            for word in query.split():
                if word.lower() in summary.lower():
                    idx = summary.lower().find(word.lower())
                    original_word = summary[idx : idx + len(word)]
                    summary = summary.replace(
                        original_word,
                        f"{SEARCH_HIGHLIGHT}{original_word}{RESET}",
                        1,
                    )

        cat_str = f" [{event.category}]" if event.category else ""
        print(
            f"  {_c(CYAN, date_str)}  "
            f"{_c(DIM, event.event_type)}{_c(DIM, cat_str)}"
        )
        print(f"    {summary}")

        tags_str = ""
        if event.tags:
            tags_str = " ".join(f"#{t}" for t in event.tags)
            print(f"    {_c(DIM, tags_str)}")
        print()

    print(_c(DIM, f"  Найдено: {len(events)}"))


def cmd_stats(db: Ark, args: argparse.Namespace) -> None:
    """Show database statistics."""
    stats = db.get_stats()

    print(_c(BOLD, "Ark Life DB"))
    print(f"  Файл:       {db.db_path}")
    print(f"  Размер:     {_format_size(stats['file_size_bytes'])}")
    print(f"  Событий:    {stats['events_count']:,}")
    print(f"  Сущностей:  {stats['entities_count']:,}")

    if stats["categories"]:
        print()
        print(_c(BOLD, "Категории:"))
        for cat, count in sorted(
            stats["categories"].items(), key=lambda x: -x[1]
        ):
            bar_len = min(count * 30 // max(stats["categories"].values()), 30)
            bar = "\u2588" * bar_len
            print(f"  {cat:<16} {count:>5,}  {_c(GREEN, bar)}")

    if stats["event_types"]:
        print()
        print(_c(BOLD, f"Типы событий ({len(stats['event_types'])}):"))
        sorted_types = sorted(
            stats["event_types"].items(), key=lambda x: -x[1]
        )
        max_count = max(c for _, c in sorted_types) if sorted_types else 1
        for etype, count in sorted_types:
            bar_len = min(count * 20 // max_count, 20)
            bar = "\u2591" * bar_len
            print(f"  {etype:<22} {count:>4,}  {_c(DIM, bar)}")


def cmd_sync_conflicts(db: Ark, args: argparse.Namespace) -> None:
    """Show unresolved sync conflicts."""
    from core.sync import SyncManager

    # We only need to read conflicts, so use a minimal SyncManager
    # Just connect directly to check if sync tables exist
    import sqlite3

    try:
        conn = sqlite3.connect(db.db_path)
        conn.row_factory = sqlite3.Row
        row = conn.execute(
            "SELECT 1 FROM sqlite_master WHERE type='table' AND name='sync_conflicts'"
        ).fetchone()
        if not row:
            print(_c(DIM, "Sync не настроен (таблицы sync отсутствуют)."))
            conn.close()
            return

        rows = conn.execute(
            "SELECT * FROM sync_conflicts WHERE resolved = 0 ORDER BY created_at DESC"
        ).fetchall()
        conn.close()
    except sqlite3.Error as e:
        print(f"Ошибка БД: {e}", file=sys.stderr)
        sys.exit(1)

    if not rows:
        print(_c(GREEN, "Нет неразрешенных конфликтов."))
        return

    print(_c(BOLD, f"Неразрешенные конфликты ({len(rows)}):"))
    print()
    for row in rows:
        conflict_id = row["id"][:8]
        event_id = row["event_id"][:8]
        local_dev = row["local_device"]
        remote_dev = row["remote_device"]
        created = _format_timestamp(row["created_at"]) if row["created_at"] else "?"

        print(f"  {_c(CYAN, conflict_id)}  event={event_id}")
        print(f"    local: {local_dev}  remote: {remote_dev}")
        print(f"    создан: {created}")
        print()

    print(
        _c(DIM, "  Для разрешения: ark sync resolve <id> --keep local|remote")
    )


def cmd_sync_resolve(db: Ark, args: argparse.Namespace) -> None:
    """Resolve a sync conflict."""
    from core.sync import SyncManager

    import sqlite3

    conflict_id = args.id
    keep = args.keep

    if keep not in ("local", "remote"):
        print("Ошибка: --keep должен быть 'local' или 'remote'", file=sys.stderr)
        sys.exit(1)

    try:
        conn = sqlite3.connect(db.db_path)
        conn.row_factory = sqlite3.Row

        # Find conflict by prefix match
        row = conn.execute(
            "SELECT id FROM sync_conflicts WHERE id LIKE ? AND resolved = 0",
            (f"{conflict_id}%",),
        ).fetchone()

        if not row:
            print(f"Конфликт '{conflict_id}' не найден.", file=sys.stderr)
            conn.close()
            sys.exit(1)

        full_id = row["id"]
        conn.execute(
            "UPDATE sync_conflicts SET resolved = 1, resolution = ? WHERE id = ?",
            (keep, full_id),
        )
        conn.commit()
        conn.close()

        print(f"{_c(GREEN, '+')} Конфликт {full_id[:8]} разрешен: keep={keep}")
    except sqlite3.Error as e:
        print(f"Ошибка БД: {e}", file=sys.stderr)
        sys.exit(1)


def cmd_serve(db: Ark, args: argparse.Namespace) -> None:
    """Start the FastAPI server."""
    try:
        import uvicorn
    except ImportError:
        print(
            "Ошибка: uvicorn не установлен. Установите: pip install uvicorn",
            file=sys.stderr,
        )
        sys.exit(1)

    host = args.host if hasattr(args, "host") else "0.0.0.0"
    port = args.port if hasattr(args, "port") else 8000

    # Set DB path env so server picks it up
    os.environ["LIFE_DB_PATH"] = str(db.db_path)

    print(f"Запуск Ark сервера на {host}:{port}")
    print(f"  БД: {db.db_path}")
    uvicorn.run("server.app:app", host=host, port=port, reload=False)


# ============================================================================
# Parser
# ============================================================================


def build_parser() -> argparse.ArgumentParser:
    parser = argparse.ArgumentParser(
        prog="ark",
        description="Ark Life DB \u2014 CLI для персональной базы данных жизни",
    )
    parser.add_argument(
        "--db",
        type=str,
        default=None,
        help=f"Путь к БД (по умолчанию: $LIFE_DB_PATH или {DEFAULT_DB_PATH})",
    )

    sub = parser.add_subparsers(dest="command", help="Команда")

    # --- add ---
    p_add = sub.add_parser("add", help="Добавить событие")
    p_add.add_argument("summary", help="Описание события")
    p_add.add_argument("--type", "-t", required=True, help="Тип события")
    p_add.add_argument("--category", "-c", default=None, help="Категория")
    p_add.add_argument("--tags", default=None, help="Теги через запятую")
    p_add.add_argument("--data", default=None, help="JSON данные события")

    # --- task ---
    p_task = sub.add_parser("task", help="Добавить задачу (shortcut)")
    p_task.add_argument("summary", help="Описание задачи")
    p_task.add_argument("--tags", default=None, help="Теги через запятую")

    # --- note ---
    p_note = sub.add_parser("note", help="Добавить заметку (shortcut)")
    p_note.add_argument("summary", help="Текст заметки")
    p_note.add_argument("--tags", default=None, help="Теги через запятую")

    # --- health ---
    p_health = sub.add_parser("health", help="Добавить событие здоровья (shortcut)")
    p_health.add_argument("summary", help="Описание")
    p_health.add_argument("--type", "-t", default=None, help="Тип (weight, workout, ...)")
    p_health.add_argument("--tags", default=None, help="Теги через запятую")

    # --- list ---
    p_list = sub.add_parser("list", help="Список событий")
    p_list.add_argument("--category", "-c", default=None, help="Фильтр по категории")
    p_list.add_argument("--type", "-t", default=None, help="Фильтр по типу")
    p_list.add_argument("--since", default=None, help="С даты (YYYY-MM-DD)")
    p_list.add_argument("--source", default=None, help="Фильтр по источнику")
    p_list.add_argument("--tags", default=None, help="Фильтр по тегам (через запятую)")
    p_list.add_argument(
        "--limit", "-n", type=int, default=DEFAULT_LIMIT, help=f"Макс. количество (по умолчанию: {DEFAULT_LIMIT})"
    )

    # --- search ---
    p_search = sub.add_parser("search", help="Полнотекстовый поиск")
    p_search.add_argument("query", help="Поисковый запрос")
    p_search.add_argument("--limit", "-n", type=int, default=20, help="Макс. результатов")

    # --- stats ---
    sub.add_parser("stats", help="Статистика базы данных")

    # --- sync ---
    p_sync = sub.add_parser("sync", help="Управление синхронизацией")
    sync_sub = p_sync.add_subparsers(dest="sync_command")

    sync_sub.add_parser("conflicts", help="Показать неразрешенные конфликты")

    p_resolve = sync_sub.add_parser("resolve", help="Разрешить конфликт")
    p_resolve.add_argument("id", help="ID конфликта (можно префикс)")
    p_resolve.add_argument(
        "--keep", required=True, choices=["local", "remote"], help="Какую версию оставить"
    )

    # --- serve ---
    p_serve = sub.add_parser("serve", help="Запустить FastAPI сервер")
    p_serve.add_argument("--host", default="0.0.0.0", help="Хост (по умолчанию: 0.0.0.0)")
    p_serve.add_argument("--port", "-p", type=int, default=8000, help="Порт (по умолчанию: 8000)")

    return parser


# ============================================================================
# Main
# ============================================================================


def main(argv: Optional[list[str]] = None) -> None:
    parser = build_parser()
    args = parser.parse_args(argv)

    if not args.command:
        parser.print_help()
        sys.exit(0)

    # Resolve DB path
    db_path = Path(args.db) if args.db else DEFAULT_DB_PATH
    create = args.command in ("add", "task", "note", "health", "serve")

    try:
        db = Ark(db_path, create=create)
    except FileNotFoundError:
        print(
            f"Ошибка: БД не найдена: {db_path}\n"
            f"Создайте новую: ark add 'первое событие' --type init -c system",
            file=sys.stderr,
        )
        sys.exit(1)
    except Exception as e:
        print(f"Ошибка открытия БД: {e}", file=sys.stderr)
        sys.exit(1)

    # Dispatch
    if args.command == "add":
        cmd_add(db, args)
    elif args.command == "task":
        cmd_task(db, args)
    elif args.command == "note":
        cmd_note(db, args)
    elif args.command == "health":
        cmd_health(db, args)
    elif args.command == "list":
        cmd_list(db, args)
    elif args.command == "search":
        cmd_search(db, args)
    elif args.command == "stats":
        cmd_stats(db, args)
    elif args.command == "sync":
        if args.sync_command == "conflicts":
            cmd_sync_conflicts(db, args)
        elif args.sync_command == "resolve":
            cmd_sync_resolve(db, args)
        else:
            print("Используйте: ark sync conflicts | ark sync resolve <id> --keep local|remote")
    elif args.command == "serve":
        cmd_serve(db, args)
    else:
        parser.print_help()


if __name__ == "__main__":
    main()
