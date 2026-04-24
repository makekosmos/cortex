from __future__ import annotations

import json
import os
import pathlib
import sqlite3
import sys
from typing import Any


GAME_OBJECT_TYPE_ID = "game_obj"


def resolve_app_data_path() -> pathlib.Path:
    if os.name == "nt":
        return pathlib.Path(os.environ.get("APPDATA", pathlib.Path.home() / "AppData" / "Roaming"))
    if sys.platform == "darwin":
        return pathlib.Path.home() / "Library" / "Application Support"
    return pathlib.Path(os.environ.get("XDG_CONFIG_HOME", pathlib.Path.home() / ".config"))


def get_selected_target_db() -> pathlib.Path:
    app_data = resolve_app_data_path()
    selected_path = app_data / "Kepler" / "selected-space.json"
    if not selected_path.exists():
        return app_data / "Kepler" / "ark.db"

    selection = json.loads(selected_path.read_text(encoding="utf-8"))
    space_id = selection.get("spaceId")
    if not isinstance(space_id, str) or not space_id:
        return app_data / "Kepler" / "ark.db"

    return app_data / "Kepler" / "spaces" / space_id / "ark.db"


def normalize_exe_path(exe_path: str) -> str:
    return exe_path.strip().replace("/", "\\").lower()


def parse_props(value: str | None) -> dict[str, Any]:
    if not value:
        return {}
    try:
        parsed = json.loads(value)
    except json.JSONDecodeError:
        return {}
    return parsed if isinstance(parsed, dict) else {}


def read_optional_string(value: Any) -> str | None:
    if not isinstance(value, str):
        return None
    trimmed = value.strip()
    return trimmed if trimmed else None


def ensure_game_object_type(source: sqlite3.Connection, target: sqlite3.Connection) -> int:
    target_row = target.execute(
        "SELECT id FROM object_types WHERE id = ?",
        (GAME_OBJECT_TYPE_ID,),
    ).fetchone()
    if target_row is not None:
        return 0

    source_row = source.execute(
        """
        SELECT id, name, schema_json, ui_schema_json, created_at, updated_at, system_locked
        FROM object_types
        WHERE id = ?
        """,
        (GAME_OBJECT_TYPE_ID,),
    ).fetchone()
    if source_row is None:
        return 0

    target.execute(
        """
        INSERT INTO object_types
          (id, name, schema_json, ui_schema_json, created_at, updated_at, system_locked)
        VALUES (?, ?, ?, ?, ?, ?, ?)
        """,
        tuple(source_row),
    )
    return 1


def migrate(source_path: pathlib.Path, target_path: pathlib.Path) -> dict[str, int]:
    if source_path.resolve() == target_path.resolve():
        raise RuntimeError("Source and target Ark DB must be different")

    source = sqlite3.connect(source_path)
    target = sqlite3.connect(target_path)
    source.row_factory = sqlite3.Row
    target.row_factory = sqlite3.Row

    try:
        result = {
            "migratedTypes": 0,
            "migratedObjects": 0,
            "mergedObjects": 0,
            "skippedObjects": 0,
        }

        target.execute("BEGIN IMMEDIATE TRANSACTION")
        result["migratedTypes"] = ensure_game_object_type(source, target)

        source_rows = source.execute(
            """
            SELECT id, title, content_json, props_json, created_at, updated_at
            FROM objects
            WHERE type_id = ? AND deleted_at IS NULL
            ORDER BY updated_at DESC, created_at DESC
            """,
            (GAME_OBJECT_TYPE_ID,),
        ).fetchall()

        target_rows = target.execute(
            """
            SELECT id, title, content_json, props_json, created_at, updated_at
            FROM objects
            WHERE type_id = ? AND deleted_at IS NULL
            ORDER BY updated_at DESC, created_at DESC
            """,
            (GAME_OBJECT_TYPE_ID,),
        ).fetchall()

        target_by_id: dict[str, sqlite3.Row] = {}
        target_by_game_id: dict[str, sqlite3.Row] = {}
        target_by_exe_path: dict[str, sqlite3.Row] = {}

        for row in target_rows:
            target_by_id[row["id"]] = row
            props = parse_props(row["props_json"])
            game_id = read_optional_string(props.get("arrancador_game_id"))
            if game_id and game_id not in target_by_game_id:
                target_by_game_id[game_id] = row
            exe_path = read_optional_string(props.get("exe_path"))
            if exe_path:
                normalized = normalize_exe_path(exe_path)
                if normalized not in target_by_exe_path:
                    target_by_exe_path[normalized] = row

        for source_row in source_rows:
            source_props = parse_props(source_row["props_json"])
            source_game_id = read_optional_string(source_props.get("arrancador_game_id"))
            source_exe_path = read_optional_string(source_props.get("exe_path"))

            target_match = (
                target_by_id.get(source_row["id"])
                or (target_by_game_id.get(source_game_id) if source_game_id else None)
                or (target_by_exe_path.get(normalize_exe_path(source_exe_path)) if source_exe_path else None)
            )

            target_props = parse_props(target_match["props_json"]) if target_match else {}
            merged_props = {**target_props, **source_props}
            object_id = target_match["id"] if target_match else source_row["id"]
            created_at = (
                min(target_match["created_at"], source_row["created_at"])
                if target_match
                else source_row["created_at"]
            )
            updated_at = (
                max(target_match["updated_at"], source_row["updated_at"])
                if target_match
                else source_row["updated_at"]
            )

            if (
                target_match
                and target_match["title"] == source_row["title"]
                and target_match["content_json"] == source_row["content_json"]
                and json.dumps(parse_props(target_match["props_json"]), sort_keys=True)
                == json.dumps(merged_props, sort_keys=True)
            ):
                result["skippedObjects"] += 1
                continue

            target.execute(
                """
                INSERT OR REPLACE INTO objects
                  (id, type_id, title, content_json, props_json, created_at, updated_at, deleted_at)
                VALUES (?, ?, ?, ?, ?, ?, ?, NULL)
                """,
                (
                    object_id,
                    GAME_OBJECT_TYPE_ID,
                    source_row["title"],
                    source_row["content_json"],
                    json.dumps(merged_props, ensure_ascii=False),
                    created_at,
                    updated_at,
                ),
            )

            result["mergedObjects" if target_match else "migratedObjects"] += 1

        target.commit()
        return result
    except Exception:
        target.rollback()
        raise
    finally:
        source.close()
        target.close()


def main() -> int:
    if len(sys.argv) < 2:
        raise RuntimeError(
            "Usage: bun run migrate:ark-games -- <source-ark-db-path> [target-ark-db-path]"
        )

    source_path = pathlib.Path(sys.argv[1]).expanduser().resolve()
    target_path = (
        pathlib.Path(sys.argv[2]).expanduser().resolve()
        if len(sys.argv) > 2
        else get_selected_target_db().resolve()
    )

    result = migrate(source_path, target_path)
    print(
        json.dumps(
            {
                "sourceDbPath": str(source_path),
                "targetDbPath": str(target_path),
                **result,
            },
            ensure_ascii=False,
            indent=2,
        )
    )
    return 0


if __name__ == "__main__":
    try:
        raise SystemExit(main())
    except Exception as error:
        print(f"Failed to migrate Ark games: {error}", file=sys.stderr)
        raise SystemExit(1)
