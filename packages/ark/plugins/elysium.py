"""
Elysium Plugin for Ark

Imports nutrition, water, and food data from the Elysium app's SQLite database
into the Ark Life DB.

Elysium is a React Native (Expo) nutrition tracker that stores data in a local
SQLite database (elysium.db) with tables: nutrition_entries, water_entries,
custom_foods, recent_foods, settings.

Usage:
    from plugins.elysium import ElysiumPlugin

    plugin = ElysiumPlugin("/path/to/elysium.db")
    plugin.sync_to_ark(db)            # Full sync
    plugin.sync_to_ark(db, days=30)   # Last 30 days
"""

from __future__ import annotations

import json
import sqlite3
from dataclasses import dataclass, field
from datetime import datetime, timedelta, timezone
from pathlib import Path
from typing import Any, Optional, Union

from core.ark import Ark, parse_iso8601, to_iso8601, utc_now

# ============================================================================
# Data Classes
# ============================================================================


@dataclass
class NutritionEntry:
    """A single nutrition log entry from Elysium."""

    id: str
    date: str  # YYYY-MM-DD
    food_json: str  # JSON-encoded FoodItem
    quantity: float
    meal_type: str  # breakfast | lunch | dinner | snack
    logged_at: str  # ISO 8601

    # Parsed from food_json
    food_name: str = ""
    food_brand: Optional[str] = None
    serving_size: float = 0
    serving_unit: str = ""
    calories: float = 0
    protein: float = 0
    fat: float = 0
    carbs: float = 0

    @classmethod
    def from_row(cls, row: sqlite3.Row) -> NutritionEntry:
        """Create from Elysium SQLite row."""
        entry = cls(
            id=row["id"],
            date=row["date"],
            food_json=row["food_json"],
            quantity=row["quantity"],
            meal_type=row["meal_type"],
            logged_at=row["logged_at"],
        )

        # Parse food JSON to extract macros
        try:
            food = json.loads(row["food_json"])
            entry.food_name = food.get("name", "")
            entry.food_brand = food.get("brand")
            entry.serving_size = food.get("servingSize", 0)
            entry.serving_unit = food.get("servingUnit", "")
            macros = food.get("macros", {})
            entry.calories = macros.get("calories", 0)
            entry.protein = macros.get("protein", 0)
            entry.fat = macros.get("fat", 0)
            entry.carbs = macros.get("carbs", 0)
        except (json.JSONDecodeError, TypeError):
            pass

        return entry


@dataclass
class WaterEntry:
    """A single water intake entry from Elysium."""

    id: str
    date: str  # YYYY-MM-DD
    amount: int  # ml
    time: str  # ISO 8601

    @classmethod
    def from_row(cls, row: sqlite3.Row) -> WaterEntry:
        """Create from Elysium SQLite row."""
        return cls(
            id=row["id"],
            date=row["date"],
            amount=row["amount"],
            time=row["time"],
        )


@dataclass
class DailyNutritionSummary:
    """Aggregated daily nutrition totals."""

    date: str
    total_calories: float
    total_protein: float
    total_fat: float
    total_carbs: float
    meal_count: int
    meals: dict[str, int] = field(default_factory=dict)  # meal_type -> count


# ============================================================================
# Elysium Database Reader
# ============================================================================


class ElysiumDB:
    """
    Read-only client for the Elysium SQLite database.

    Elysium schema:
        nutrition_entries(id, date, food_json, quantity, meal_type, logged_at)
        water_entries(id, date, amount, time)
        custom_foods(id, name, brand, serving_size, serving_unit, macros_json)
        recent_foods(id, food_json, used_at)
        settings(key, value)
    """

    def __init__(self, db_path: Union[str, Path]):
        self.db_path = Path(db_path)
        if not self.db_path.exists():
            raise FileNotFoundError(f"Elysium database not found: {self.db_path}")

    def _connect(self) -> sqlite3.Connection:
        """Open a read-only connection."""
        conn = sqlite3.connect(
            f"file:{self.db_path}?mode=ro",
            uri=True,
            detect_types=sqlite3.PARSE_DECLTYPES | sqlite3.PARSE_COLNAMES,
            timeout=10,
        )
        conn.row_factory = sqlite3.Row
        return conn

    def get_nutrition_entries(
        self,
        start_date: Optional[str] = None,
        end_date: Optional[str] = None,
    ) -> list[NutritionEntry]:
        """
        Get nutrition entries, optionally filtered by date range.

        Args:
            start_date: Start date (YYYY-MM-DD), inclusive
            end_date: End date (YYYY-MM-DD), inclusive
        """
        conditions = []
        params: list[str] = []

        if start_date:
            conditions.append("date >= ?")
            params.append(start_date)
        if end_date:
            conditions.append("date <= ?")
            params.append(end_date)

        where = f"WHERE {' AND '.join(conditions)}" if conditions else ""
        query = f"SELECT * FROM nutrition_entries {where} ORDER BY logged_at"  # noqa: S608

        conn = self._connect()
        try:
            rows = conn.execute(query, params).fetchall()
            return [NutritionEntry.from_row(r) for r in rows]
        finally:
            conn.close()

    def get_water_entries(
        self,
        start_date: Optional[str] = None,
        end_date: Optional[str] = None,
    ) -> list[WaterEntry]:
        """
        Get water entries, optionally filtered by date range.

        Args:
            start_date: Start date (YYYY-MM-DD), inclusive
            end_date: End date (YYYY-MM-DD), inclusive
        """
        conditions = []
        params: list[str] = []

        if start_date:
            conditions.append("date >= ?")
            params.append(start_date)
        if end_date:
            conditions.append("date <= ?")
            params.append(end_date)

        where = f"WHERE {' AND '.join(conditions)}" if conditions else ""
        query = f"SELECT * FROM water_entries {where} ORDER BY time"  # noqa: S608

        conn = self._connect()
        try:
            rows = conn.execute(query, params).fetchall()
            return [WaterEntry.from_row(r) for r in rows]
        finally:
            conn.close()

    def get_daily_nutrition_summaries(
        self,
        start_date: Optional[str] = None,
        end_date: Optional[str] = None,
    ) -> list[DailyNutritionSummary]:
        """
        Compute daily aggregated nutrition summaries from individual entries.
        """
        entries = self.get_nutrition_entries(start_date, end_date)

        by_date: dict[str, list[NutritionEntry]] = {}
        for entry in entries:
            by_date.setdefault(entry.date, []).append(entry)

        summaries = []
        for date, day_entries in sorted(by_date.items()):
            total_cal = 0.0
            total_pro = 0.0
            total_fat = 0.0
            total_carbs = 0.0
            meals: dict[str, int] = {}

            for e in day_entries:
                q = e.quantity
                total_cal += e.calories * q
                total_pro += e.protein * q
                total_fat += e.fat * q
                total_carbs += e.carbs * q
                meals[e.meal_type] = meals.get(e.meal_type, 0) + 1

            summaries.append(
                DailyNutritionSummary(
                    date=date,
                    total_calories=round(total_cal, 1),
                    total_protein=round(total_pro, 1),
                    total_fat=round(total_fat, 1),
                    total_carbs=round(total_carbs, 1),
                    meal_count=len(day_entries),
                    meals=meals,
                )
            )

        return summaries

    def get_setting(self, key: str) -> Optional[str]:
        """Read a setting from the Elysium settings table."""
        conn = self._connect()
        try:
            row = conn.execute(
                "SELECT value FROM settings WHERE key = ?", (key,)
            ).fetchone()
            return row["value"] if row else None
        finally:
            conn.close()


# ============================================================================
# Elysium Plugin
# ============================================================================


class ElysiumPlugin:
    """
    Plugin to import Elysium nutrition data into Ark.

    Event types produced:
        food_consumed     — individual meal/food log entry
        daily_nutrition   — aggregated daily macro totals
        water_intake      — individual water log entry

    All events use:
        category: "nutrition"
        source: "elysium"
        source_id: "<elysium_entry_id>" (for food/water)
                   "daily_<date>" (for daily summaries)
    """

    SOURCE = "elysium"
    CATEGORY = "nutrition"

    # Event types
    FOOD_CONSUMED = "food_consumed"
    DAILY_NUTRITION = "daily_nutrition"
    WATER_INTAKE = "water_intake"

    def __init__(self, elysium_db_path: Union[str, Path]):
        """
        Initialize plugin.

        Args:
            elysium_db_path: Path to the Elysium SQLite database (elysium.db)
        """
        self.db = ElysiumDB(elysium_db_path)

    # ────────────────────────────────────────────────────────────────────
    # Converters
    # ────────────────────────────────────────────────────────────────────

    def _convert_nutrition_entry(self, entry: NutritionEntry) -> dict[str, Any]:
        """Convert a single nutrition entry to an Ark event dict."""
        data: dict[str, Any] = {
            "food_name": entry.food_name,
            "meal_type": entry.meal_type,
            "quantity": entry.quantity,
            "serving_size": entry.serving_size,
            "serving_unit": entry.serving_unit,
            "calories": round(entry.calories * entry.quantity, 1),
            "protein": round(entry.protein * entry.quantity, 1),
            "fat": round(entry.fat * entry.quantity, 1),
            "carbs": round(entry.carbs * entry.quantity, 1),
            # Per-serving macros for reference
            "per_serving": {
                "calories": entry.calories,
                "protein": entry.protein,
                "fat": entry.fat,
                "carbs": entry.carbs,
            },
        }

        if entry.food_brand:
            data["brand"] = entry.food_brand

        # Build summary: "Овсянка x2 (breakfast) — 340 kcal"
        parts = [entry.food_name]
        if entry.quantity != 1:
            parts.append(f"x{entry.quantity:g}")
        parts.append(f"({entry.meal_type})")
        parts.append(f"— {round(entry.calories * entry.quantity)} kcal")
        summary = " ".join(parts)

        return {
            "event_type": self.FOOD_CONSUMED,
            "category": self.CATEGORY,
            "occurred_at": entry.logged_at,
            "data": data,
            "summary": summary,
            "source": self.SOURCE,
            "source_id": entry.id,
            "tags": [entry.meal_type],
        }

    def _convert_daily_summary(self, summary: DailyNutritionSummary) -> dict[str, Any]:
        """Convert a daily nutrition summary to an Ark event dict."""
        data: dict[str, Any] = {
            "calories": summary.total_calories,
            "protein": summary.total_protein,
            "fat": summary.total_fat,
            "carbs": summary.total_carbs,
            "meal_count": summary.meal_count,
            "meals": summary.meals,
        }

        summary_text = (
            f"{round(summary.total_calories)} kcal — "
            f"P:{round(summary.total_protein)}g "
            f"F:{round(summary.total_fat)}g "
            f"C:{round(summary.total_carbs)}g "
            f"({summary.meal_count} entries)"
        )

        # Use noon of the date as the event time
        occurred_at = f"{summary.date}T12:00:00Z"

        return {
            "event_type": self.DAILY_NUTRITION,
            "category": self.CATEGORY,
            "occurred_at": occurred_at,
            "data": data,
            "summary": summary_text,
            "source": self.SOURCE,
            "source_id": f"daily_{summary.date}",
            "tags": ["daily_summary"],
        }

    def _convert_water_entry(self, entry: WaterEntry) -> dict[str, Any]:
        """Convert a water intake entry to an Ark event dict."""
        return {
            "event_type": self.WATER_INTAKE,
            "category": self.CATEGORY,
            "occurred_at": entry.time,
            "data": {
                "amount_ml": entry.amount,
                "date": entry.date,
            },
            "summary": f"{entry.amount} ml",
            "source": self.SOURCE,
            "source_id": f"water_{entry.id}",
            "tags": ["water"],
        }

    # ────────────────────────────────────────────────────────────────────
    # Date range helpers
    # ────────────────────────────────────────────────────────────────────

    def _resolve_date_range(
        self,
        ark: Ark,
        days: Optional[int] = None,
        start_date: Optional[str] = None,
        end_date: Optional[str] = None,
        full_sync: bool = False,
    ) -> tuple[Optional[str], Optional[str]]:
        """
        Determine the (start_date, end_date) strings for the sync.

        Returns (None, None) for a full sync with no date filtering.
        """
        if full_sync:
            return None, None

        if days is not None:
            dt = datetime.now(timezone.utc) - timedelta(days=days)
            return dt.strftime("%Y-%m-%d"), None

        if start_date:
            return start_date, end_date

        # Incremental: find last synced date
        events = ark.query_events(source=self.SOURCE, limit=1, order="DESC")
        if events:
            last_date = events[0].occurred_at[:10]
            dt = datetime.strptime(last_date, "%Y-%m-%d")
            return (dt - timedelta(days=1)).strftime("%Y-%m-%d"), None

        # No previous data — full sync
        return None, None

    # ────────────────────────────────────────────────────────────────────
    # Sync methods
    # ────────────────────────────────────────────────────────────────────

    def sync_nutrition(
        self,
        ark: Ark,
        start_date: Optional[str] = None,
        end_date: Optional[str] = None,
    ) -> tuple[int, int, int]:
        """
        Sync individual food_consumed events.

        Returns:
            (created, updated, skipped)
        """
        entries = self.db.get_nutrition_entries(start_date, end_date)
        if not entries:
            return (0, 0, 0)

        events = [self._convert_nutrition_entry(e) for e in entries]
        return ark.record_events_batch(events, source=self.SOURCE)

    def sync_daily_summaries(
        self,
        ark: Ark,
        start_date: Optional[str] = None,
        end_date: Optional[str] = None,
    ) -> tuple[int, int, int]:
        """
        Sync daily_nutrition summary events.

        Returns:
            (created, updated, skipped)
        """
        summaries = self.db.get_daily_nutrition_summaries(start_date, end_date)
        if not summaries:
            return (0, 0, 0)

        events = [self._convert_daily_summary(s) for s in summaries]
        return ark.record_events_batch(events, source=self.SOURCE)

    def sync_water(
        self,
        ark: Ark,
        start_date: Optional[str] = None,
        end_date: Optional[str] = None,
    ) -> tuple[int, int, int]:
        """
        Sync water_intake events.

        Returns:
            (created, updated, skipped)
        """
        entries = self.db.get_water_entries(start_date, end_date)
        if not entries:
            return (0, 0, 0)

        events = [self._convert_water_entry(e) for e in entries]
        return ark.record_events_batch(events, source=self.SOURCE)

    def sync_to_ark(
        self,
        ark: Ark,
        days: Optional[int] = None,
        start_date: Optional[str] = None,
        end_date: Optional[str] = None,
        full_sync: bool = False,
    ) -> dict[str, tuple[int, int, int]]:
        """
        Full sync: nutrition entries + daily summaries + water.

        Args:
            ark: Ark database instance
            days: Sync last N days
            start_date: Start date (YYYY-MM-DD)
            end_date: End date (YYYY-MM-DD)
            full_sync: If True, sync all data (no date filter)

        Returns:
            Dict mapping event type to (created, updated, skipped) tuples:
            {
                "food_consumed": (c, u, s),
                "daily_nutrition": (c, u, s),
                "water_intake": (c, u, s),
            }
        """
        sd, ed = self._resolve_date_range(ark, days, start_date, end_date, full_sync)

        # Start import tracking
        import_id = ark.start_import(
            source=self.SOURCE,
            file_name=f"elysium_sync_{datetime.now(timezone.utc).strftime('%Y%m%d_%H%M%S')}",
        )

        try:
            results: dict[str, tuple[int, int, int]] = {}

            results[self.FOOD_CONSUMED] = self.sync_nutrition(ark, sd, ed)
            results[self.DAILY_NUTRITION] = self.sync_daily_summaries(ark, sd, ed)
            results[self.WATER_INTAKE] = self.sync_water(ark, sd, ed)

            # Totals for import tracking
            total_created = sum(r[0] for r in results.values())
            total_updated = sum(r[1] for r in results.values())
            total_skipped = sum(r[2] for r in results.values())

            # Calculate date range for import record
            date_range_start = sd or "1970-01-01"
            date_range_end = ed or datetime.now(timezone.utc).strftime("%Y-%m-%d")

            ark.complete_import(
                import_id,
                records_created=total_created,
                records_updated=total_updated,
                records_skipped=total_skipped,
                date_range_start=f"{date_range_start}T00:00:00Z",
                date_range_end=f"{date_range_end}T23:59:59Z",
            )

            return results

        except Exception as e:
            ark.fail_import(import_id, str(e))
            raise
