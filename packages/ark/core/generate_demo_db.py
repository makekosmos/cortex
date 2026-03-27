#!/usr/bin/env python3
"""Generate a demo database with sample data for testing the desktop UI."""

import argparse
import json
import random
import sqlite3
import uuid
from datetime import datetime, timedelta
from pathlib import Path
from typing import Optional, Tuple


def generate_uuid() -> str:
    return str(uuid.uuid4())


def random_datetime(start: datetime, end: datetime) -> datetime:
    delta = end - start
    random_seconds = random.randint(0, int(delta.total_seconds()))
    return start + timedelta(seconds=random_seconds)


def format_dt(dt: datetime) -> str:
    return dt.strftime("%Y-%m-%dT%H:%M:%SZ")


# Categories and their event types with generation functions
CATEGORIES = {
    "health": {
        "event_types": [
            "heart_rate",
            "steps",
            "sleep",
            "weight",
            "blood_pressure",
            "workout",
        ],
        "weight": 35,  # higher = more events
    },
    "media": {
        "event_types": [
            "video_watched",
            "music_played",
            "book_read",
            "article_read",
            "podcast_listened",
        ],
        "weight": 25,
    },
    "finance": {
        "event_types": ["transaction", "subscription", "salary", "investment"],
        "weight": 15,
    },
    "social": {
        "event_types": ["message_sent", "call", "meeting", "social_post"],
        "weight": 10,
    },
    "productivity": {
        "event_types": [
            "task_completed",
            "document_created",
            "code_commit",
            "email_sent",
        ],
        "weight": 10,
    },
    "location": {
        "event_types": ["place_visited", "trip", "commute"],
        "weight": 5,
    },
}


def generate_health_data(event_type: str) -> Tuple[dict, Optional[str]]:
    """Generate health-related event data."""
    if event_type == "heart_rate":
        bpm = random.randint(55, 120)
        return {
            "bpm": bpm,
            "context": random.choice(["resting", "walking", "exercise"]),
        }, f"Heart rate: {bpm} bpm"
    elif event_type == "steps":
        steps = random.randint(100, 15000)
        return {
            "count": steps,
            "distance_km": round(steps * 0.0008, 2),
        }, f"Walked {steps} steps"
    elif event_type == "sleep":
        hours = round(random.uniform(4, 10), 1)
        quality = random.choice(["poor", "fair", "good", "excellent"])
        return {
            "hours": hours,
            "quality": quality,
            "deep_sleep_pct": random.randint(10, 30),
        }, f"Slept {hours} hours ({quality})"
    elif event_type == "weight":
        kg = round(random.uniform(60, 90), 1)
        return {
            "kg": kg,
            "body_fat_pct": round(random.uniform(15, 30), 1),
        }, f"Weight: {kg} kg"
    elif event_type == "blood_pressure":
        sys = random.randint(100, 140)
        dia = random.randint(60, 90)
        return {
            "systolic": sys,
            "diastolic": dia,
            "pulse": random.randint(60, 100),
        }, f"BP: {sys}/{dia} mmHg"
    elif event_type == "workout":
        workout_type = random.choice(
            ["running", "cycling", "swimming", "weights", "yoga"]
        )
        duration = random.randint(15, 90)
        calories = duration * random.randint(5, 12)
        return {
            "type": workout_type,
            "duration_min": duration,
            "calories": calories,
        }, f"{workout_type.title()} workout: {duration} min"
    return {}, None


def generate_media_data(event_type: str) -> Tuple[dict, Optional[str]]:
    """Generate media-related event data."""
    if event_type == "video_watched":
        videos = [
            ("How to Build a REST API", "Tech Channel", 25),
            ("Understanding Neural Networks", "AI Explained", 45),
            ("Morning Yoga Routine", "Fitness Plus", 30),
            ("World News Update", "News Network", 15),
            ("Cooking Italian Pasta", "Chef's Table", 20),
            ("Documentary: Space Exploration", "Science Hub", 90),
        ]
        title, channel, duration = random.choice(videos)
        return {
            "title": title,
            "channel": channel,
            "duration_min": duration,
            "platform": "youtube",
        }, f"Watched: {title}"
    elif event_type == "music_played":
        songs = [
            ("Bohemian Rhapsody", "Queen"),
            ("Billie Jean", "Michael Jackson"),
            ("Shape of You", "Ed Sheeran"),
            ("Blinding Lights", "The Weeknd"),
            ("Hotel California", "Eagles"),
        ]
        title, artist = random.choice(songs)
        return {
            "title": title,
            "artist": artist,
            "duration_sec": random.randint(180, 360),
        }, f"Played: {title} by {artist}"
    elif event_type == "book_read":
        books = [
            ("Atomic Habits", "James Clear", 320),
            ("1984", "George Orwell", 328),
            ("The Pragmatic Programmer", "David Thomas", 352),
            ("Sapiens", "Yuval Noah Harari", 443),
        ]
        title, author, pages = random.choice(books)
        pages_read = random.randint(5, 50)
        return {
            "title": title,
            "author": author,
            "pages_read": pages_read,
            "total_pages": pages,
        }, f"Read {pages_read} pages of {title}"
    elif event_type == "article_read":
        topics = ["technology", "science", "politics", "sports", "culture"]
        topic = random.choice(topics)
        return {
            "topic": topic,
            "source": random.choice(["Medium", "HackerNews", "Reddit", "Twitter"]),
            "reading_time_min": random.randint(3, 15),
        }, f"Read article about {topic}"
    elif event_type == "podcast_listened":
        podcasts = [
            ("Tech Talk Weekly", "Technology"),
            ("The Daily", "News"),
            ("Science Friday", "Science"),
            ("How I Built This", "Business"),
        ]
        name, category = random.choice(podcasts)
        return {
            "name": name,
            "category": category,
            "episode": random.randint(1, 500),
            "duration_min": random.randint(20, 90),
        }, f"Listened to {name}"
    return {}, None


def generate_finance_data(event_type: str) -> Tuple[dict, Optional[str]]:
    """Generate finance-related event data."""
    if event_type == "transaction":
        merchants = [
            ("Starbucks", "food", 5.50),
            ("Amazon", "shopping", 45.00),
            ("Whole Foods", "groceries", 78.00),
            ("Netflix", "entertainment", 15.99),
            ("Shell", "transport", 55.00),
            ("Gym Membership", "health", 30.00),
        ]
        merchant, cat, base_amount = random.choice(merchants)
        amount = round(base_amount * random.uniform(0.8, 1.5), 2)
        return {
            "merchant": merchant,
            "amount": amount,
            "currency": "USD",
            "category": cat,
        }, f"Spent ${amount} at {merchant}"
    elif event_type == "subscription":
        subs = [
            ("Netflix", 15.99),
            ("Spotify", 9.99),
            ("GitHub", 4.00),
            ("iCloud", 2.99),
        ]
        name, amount = random.choice(subs)
        return {
            "service": name,
            "amount": amount,
            "currency": "USD",
            "billing": "monthly",
        }, f"{name} subscription: ${amount}"
    elif event_type == "salary":
        amount = random.randint(3000, 8000)
        return {
            "amount": amount,
            "currency": "USD",
            "type": "salary",
        }, f"Received salary: ${amount}"
    elif event_type == "investment":
        assets = ["AAPL", "GOOGL", "BTC", "ETH", "VOO"]
        asset = random.choice(assets)
        amount = round(random.uniform(100, 5000), 2)
        action = random.choice(["buy", "sell"])
        return {
            "asset": asset,
            "amount": amount,
            "action": action,
        }, f"{action.title()} ${amount} of {asset}"
    return {}, None


def generate_social_data(event_type: str) -> Tuple[dict, Optional[str]]:
    """Generate social-related event data."""
    contacts = ["Alice", "Bob", "Charlie", "Diana", "Eve", "Frank"]
    if event_type == "message_sent":
        contact = random.choice(contacts)
        platform = random.choice(["WhatsApp", "Telegram", "iMessage", "Slack"])
        return {
            "contact": contact,
            "platform": platform,
            "word_count": random.randint(5, 200),
        }, f"Messaged {contact} on {platform}"
    elif event_type == "call":
        contact = random.choice(contacts)
        duration = random.randint(1, 60)
        call_type = random.choice(["voice", "video"])
        return {
            "contact": contact,
            "duration_min": duration,
            "type": call_type,
        }, f"{call_type.title()} call with {contact}: {duration} min"
    elif event_type == "meeting":
        meeting_types = ["team standup", "1-on-1", "project review", "planning session"]
        meeting = random.choice(meeting_types)
        participants = random.randint(2, 10)
        return {
            "title": meeting,
            "participants": participants,
            "duration_min": random.randint(15, 60),
        }, f"Meeting: {meeting}"
    elif event_type == "social_post":
        platforms = ["Twitter", "LinkedIn", "Instagram"]
        platform = random.choice(platforms)
        return {
            "platform": platform,
            "likes": random.randint(0, 500),
            "comments": random.randint(0, 50),
        }, f"Posted on {platform}"
    return {}, None


def generate_productivity_data(event_type: str) -> Tuple[dict, Optional[str]]:
    """Generate productivity-related event data."""
    if event_type == "task_completed":
        tasks = [
            "Review PR",
            "Write documentation",
            "Fix bug",
            "Deploy to staging",
            "Update dependencies",
        ]
        task = random.choice(tasks)
        priority = random.choice(["low", "medium", "high"])
        return {
            "title": task,
            "priority": priority,
            "project": "life",
        }, f"Completed: {task}"
    elif event_type == "document_created":
        doc_types = ["report", "proposal", "notes", "specification"]
        doc_type = random.choice(doc_types)
        return {
            "type": doc_type,
            "word_count": random.randint(100, 5000),
        }, f"Created {doc_type}"
    elif event_type == "code_commit":
        files = random.randint(1, 15)
        additions = random.randint(10, 500)
        deletions = random.randint(0, 200)
        return {
            "files_changed": files,
            "additions": additions,
            "deletions": deletions,
            "repo": "life",
        }, f"Commit: +{additions}/-{deletions} in {files} files"
    elif event_type == "email_sent":
        return {
            "recipients": random.randint(1, 5),
            "subject_length": random.randint(10, 100),
            "body_length": random.randint(50, 1000),
        }, "Sent email"
    return {}, None


def generate_location_data(event_type: str) -> Tuple[dict, Optional[str]]:
    """Generate location-related event data."""
    places = [
        ("Starbucks", "cafe", 40.7128, -74.0060),
        ("Central Park", "park", 40.7829, -73.9654),
        ("Home", "residence", 40.7500, -73.9800),
        ("Office", "workplace", 40.7580, -73.9855),
        ("Gym", "fitness", 40.7600, -73.9900),
    ]
    if event_type == "place_visited":
        name, place_type, lat, lng = random.choice(places)
        return {
            "name": name,
            "type": place_type,
            "lat": lat,
            "lng": lng,
            "duration_min": random.randint(5, 180),
        }, f"Visited {name}"
    elif event_type == "trip":
        destinations = ["Paris", "Tokyo", "London", "New York", "San Francisco"]
        dest = random.choice(destinations)
        return {
            "destination": dest,
            "duration_days": random.randint(2, 14),
        }, f"Trip to {dest}"
    elif event_type == "commute":
        mode = random.choice(["walking", "driving", "public_transit", "cycling"])
        duration = random.randint(10, 60)
        return {
            "mode": mode,
            "duration_min": duration,
            "distance_km": round(duration * 0.5, 1),
        }, f"Commute by {mode}: {duration} min"
    return {}, None


GENERATORS = {
    "health": generate_health_data,
    "media": generate_media_data,
    "finance": generate_finance_data,
    "social": generate_social_data,
    "productivity": generate_productivity_data,
    "location": generate_location_data,
}


def generate_events(count: int, start_date: datetime, end_date: datetime) -> list[dict]:
    """Generate random events."""
    events = []

    # Build weighted category list
    weighted_categories = []
    for cat, info in CATEGORIES.items():
        weighted_categories.extend([cat] * info["weight"])

    for _ in range(count):
        category = random.choice(weighted_categories)
        event_type = random.choice(CATEGORIES[category]["event_types"])

        generator = GENERATORS.get(category)
        if generator:
            data, summary = generator(event_type)
        else:
            data, summary = {}, None

        occurred_at = random_datetime(start_date, end_date)

        event = {
            "id": generate_uuid(),
            "event_type": event_type,
            "category": category,
            "occurred_at": format_dt(occurred_at),
            "data": json.dumps(data),
            "summary": summary,
            "source": f"demo_{category}",
            "source_id": generate_uuid()[:8],
            "tags": json.dumps(
                random.sample(
                    ["important", "personal", "work", "health", "fun"],
                    k=random.randint(0, 2),
                )
            ),
        }
        events.append(event)

    return events


def create_database(
    db_path: Path, event_count: int = 500, overwrite: bool = False
) -> None:
    """Create the demo database."""
    if db_path.exists():
        if not overwrite:
            raise FileExistsError(f"Output already exists: {db_path}")
        db_path.unlink()

    # Read schema
    schema_path = Path(__file__).parent / "schema.sql"
    schema = schema_path.read_text()

    # Create database
    conn = sqlite3.connect(db_path)
    conn.executescript(schema)

    # Generate events
    end_date = datetime.utcnow()
    start_date = end_date - timedelta(days=90)  # Last 90 days

    events = generate_events(event_count, start_date, end_date)

    # Insert events
    cursor = conn.cursor()
    for event in events:
        cursor.execute(
            """
            INSERT INTO events (id, event_type, category, occurred_at, data, summary, source, source_id, tags)
            VALUES (:id, :event_type, :category, :occurred_at, :data, :summary, :source, :source_id, :tags)
        """,
            event,
        )

    conn.commit()

    # Get stats
    cursor.execute("SELECT COUNT(*) FROM events")
    total = cursor.fetchone()[0]

    cursor.execute(
        "SELECT category, COUNT(*) FROM events GROUP BY category ORDER BY COUNT(*) DESC"
    )
    by_category = cursor.fetchall()

    conn.close()

    print(f"Created demo database: {db_path}")
    print(f"Total events: {total}")
    print("\nEvents by category:")
    for cat, count in by_category:
        print(f"  {cat}: {count}")


if __name__ == "__main__":
    project_root = Path(__file__).resolve().parent.parent
    default_dir = (
        project_root / "examples"
        if (project_root / "examples").exists()
        else project_root
    )

    parser = argparse.ArgumentParser(description="Generate a demo SQLite database.")
    parser.add_argument(
        "--output",
        type=Path,
        default=default_dir / "demo.db",
        help="Output database path",
    )
    parser.add_argument(
        "--count",
        type=int,
        default=500,
        help="Number of events to generate",
    )
    parser.add_argument(
        "--overwrite",
        action="store_true",
        help="Overwrite output file if it already exists",
    )
    args = parser.parse_args()

    args.output.parent.mkdir(parents=True, exist_ok=True)
    create_database(args.output, event_count=args.count, overwrite=args.overwrite)
