"""
Seed Ark DB with 70 Delphi test tasks (10 per smart list type).

Usage:
    LIFE_DB_PATH=path/to/ark.db python seed_delphi.py
"""

import json
import os
import sqlite3
import uuid
from datetime import datetime, timedelta, timezone

DB_PATH = os.environ.get("LIFE_DB_PATH", "ark.db")

now = datetime.now(timezone.utc)
today = now.strftime("%Y-%m-%dT%H:%M:%SZ")
tomorrow = (now + timedelta(days=1)).strftime("%Y-%m-%dT%H:%M:%SZ")
next_week = (now + timedelta(days=7)).strftime("%Y-%m-%dT%H:%M:%SZ")
yesterday = (now - timedelta(days=1)).strftime("%Y-%m-%dT%H:%M:%SZ")

# GTD task templates: (title_prefix, overrides)
TASK_TYPES = {
    "inbox": {},  # no project, not today/someday/scheduled
    "today": {"isToday": True},
    "upcoming": {"scheduledDate": next_week},
    "someday": {"isSomeday": True},
    "completed": {"isCompleted": True, "completedAt": yesterday},
    "cancelled": {"isCancelled": True, "cancelledAt": yesterday},
    "trashed": {"isTrashed": True},
}

TITLES = {
    "inbox": [
        "Проверить почту", "Купить продукты", "Позвонить врачу",
        "Обновить резюме", "Записаться в спортзал", "Прочитать статью",
        "Оплатить счета", "Написать отзыв", "Забрать посылку", "Почистить загрузки",
    ],
    "today": [
        "Утренняя пробежка", "Код-ревью PR", "Встреча с командой",
        "Написать тесты", "Обед с коллегой", "Деплой на стейджинг",
        "Ответить на письма", "Обновить зависимости", "Стендап в 10:00", "Купить кофе",
    ],
    "upcoming": [
        "Визит к стоматологу", "Презентация проекта", "Оплатить аренду",
        "День рождения друга", "Техосмотр машины", "Сдать отчёт",
        "Конференция DevOps", "Заказать подарок", "Продлить подписку", "Уборка квартиры",
    ],
    "someday": [
        "Выучить Rust", "Поехать в Японию", "Написать блог",
        "Освоить пианино", "Прочитать SICP", "Сделать портфолио",
        "Попробовать скалолазание", "Изучить шахматы", "Собрать NAS", "Выучить японский",
    ],
    "completed": [
        "Настроить CI/CD", "Купить монитор", "Сделать бэкап",
        "Обновить macOS", "Починить велосипед", "Оплатить VPN",
        "Написать README", "Настроить линтер", "Заказать SSD", "Сделать PR",
    ],
    "cancelled": [
        "Встреча отменена", "Поездка в Питер", "Курс по ML",
        "Подписка на Figma", "Покупка дрона", "Вебинар в пятницу",
        "Замена окон", "Регистрация на хакатон", "Консультация юриста", "Аренда офиса",
    ],
    "trashed": [
        "Тестовая задача 1", "Дубликат задачи", "Старый черновик",
        "Неактуальная заметка", "Пустая задача", "Временная запись",
        "Удалить позже", "Спам от бота", "Ошибочная задача", "Тест синхронизации",
    ],
}


def make_task_data(task_type: str, title: str, index: int) -> dict:
    """Build the inner `data` JSON dict for a Delphi task event."""
    task_id = str(uuid.uuid4())
    base = {
        "id": task_id,
        "title": title,
        "notes": None,
        "priority": 0,
        "scheduledDate": None,
        "deadline": None,
        "reminderDate": None,
        "isToday": False,
        "isEvening": False,
        "isSomeday": False,
        "isCompleted": False,
        "completedAt": None,
        "isCancelled": False,
        "cancelledAt": None,
        "isTrashed": False,
        "sortOrder": index,
        "headingId": None,
        "projectId": None,
        "areaId": None,
        "tagIds": [],
        "checklistItems": [],
        "recurrenceRule": None,
        "createdAt": today,
    }
    base.update(TASK_TYPES[task_type])
    return task_id, base


def seed():
    conn = sqlite3.connect(DB_PATH)
    cur = conn.cursor()

    # --- Ensure sync_meta table exists ---
    cur.execute("""
        CREATE TABLE IF NOT EXISTS sync_meta (
            key TEXT PRIMARY KEY,
            value TEXT NOT NULL
        )
    """)

    # --- Wipe sync state ---
    cur.execute("DELETE FROM sync_outbox")
    cur.execute("DELETE FROM sync_vectors")
    cur.execute("DELETE FROM sync_devices")

    # --- Wipe all task/project events ---
    cur.execute("DELETE FROM events WHERE event_type IN ('task', 'project', 'area', 'tag')")

    # --- New server epoch ---
    new_epoch = str(uuid.uuid4())
    cur.execute("DELETE FROM sync_meta WHERE key='server_epoch'")
    cur.execute("INSERT INTO sync_meta(key, value) VALUES('server_epoch', ?)", (new_epoch,))

    # --- Insert 70 tasks ---
    count = 0
    for task_type, titles in TITLES.items():
        for i, title in enumerate(titles):
            task_id, data = make_task_data(task_type, title, i)
            event_id = str(uuid.uuid4())
            cur.execute(
                """INSERT INTO events (id, event_type, category, occurred_at, data, summary, source, source_id, is_deleted)
                   VALUES (?, 'task', 'productivity', ?, ?, ?, 'delphi-seed', ?, 0)""",
                (event_id, today, json.dumps(data), title, task_id),
            )
            count += 1

    conn.commit()

    # Verify
    total = cur.execute("SELECT COUNT(*) FROM events WHERE is_deleted=0 AND event_type='task'").fetchone()[0]
    print(f"Seeded {count} tasks (DB has {total} tasks total)")
    print(f"New server_epoch: {new_epoch}")
    print(f"DB path: {DB_PATH}")

    conn.close()


if __name__ == "__main__":
    seed()
