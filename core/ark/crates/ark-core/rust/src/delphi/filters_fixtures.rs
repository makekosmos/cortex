//! Port от `products/delphi/tests/filterService.fixtures.ts`. ОБЯЗАТЕЛЬНО
//! must match — это контракт parity между TS-baseline и Rust-port.

use super::super::types::{SmartList, TodoItem};

pub const TODAY_ISO: &str = "2026-05-15";

fn mk(id: &str) -> TodoItem {
    TodoItem {
        id: id.to_string(),
        title: id.to_string(),
        scheduled_date: None,
        is_today: false,
        is_someday: false,
        is_completed: false,
        completed_at: None,
        is_cancelled: false,
        cancelled_at: None,
        is_trashed: false,
        sort_order: 0,
        created_at: "2026-05-15T10:00:00.000Z".to_string(),
        project_id: None,
    }
}

pub fn fixtures() -> Vec<TodoItem> {
    vec![
        TodoItem {
            sort_order: 1,
            created_at: "2026-05-13T10:00:00.000Z".into(),
            ..mk("inbox-1")
        },
        TodoItem {
            sort_order: 2,
            created_at: "2026-05-14T10:00:00.000Z".into(),
            ..mk("inbox-2")
        },
        TodoItem {
            is_today: true,
            sort_order: 10,
            ..mk("today-flag")
        },
        TodoItem {
            scheduled_date: Some(TODAY_ISO.into()),
            sort_order: 5,
            ..mk("today-scheduled")
        },
        TodoItem {
            scheduled_date: Some("2026-06-01".into()),
            sort_order: 100,
            created_at: "2026-05-10T00:00:00.000Z".into(),
            ..mk("upcoming-1")
        },
        TodoItem {
            scheduled_date: Some("2026-05-20".into()),
            sort_order: 99,
            created_at: "2026-05-11T00:00:00.000Z".into(),
            ..mk("upcoming-2")
        },
        TodoItem {
            is_someday: true,
            sort_order: 50,
            ..mk("someday-1")
        },
        TodoItem {
            is_someday: true,
            sort_order: 51,
            ..mk("someday-2")
        },
        TodoItem {
            project_id: Some("proj-1".into()),
            sort_order: 7,
            scheduled_date: Some("2026-05-20".into()),
            ..mk("proj-task")
        },
        TodoItem {
            is_completed: true,
            completed_at: Some("2026-05-14T12:00:00.000Z".into()),
            ..mk("completed-1")
        },
        TodoItem {
            is_completed: true,
            completed_at: Some("2026-05-15T08:00:00.000Z".into()),
            ..mk("completed-2")
        },
        TodoItem {
            is_cancelled: true,
            cancelled_at: Some("2026-05-13T15:00:00.000Z".into()),
            ..mk("cancelled-1")
        },
        TodoItem {
            is_trashed: true,
            created_at: "2026-05-12T10:00:00.000Z".into(),
            ..mk("trash-1")
        },
        TodoItem {
            is_trashed: true,
            created_at: "2026-05-15T11:00:00.000Z".into(),
            ..mk("trash-2")
        },
        TodoItem {
            scheduled_date: Some(TODAY_ISO.into()),
            is_completed: true,
            completed_at: Some("2026-05-15T10:00:00.000Z".into()),
            ..mk("scheduled-today-completed")
        },
        TodoItem {
            is_completed: true,
            completed_at: Some("2026-05-14T10:00:00.000Z".into()),
            is_trashed: true,
            ..mk("trashed-completed")
        },
    ]
}

/// Точно соответствует TS `EXPECTED` per SmartList.
pub fn expected_filter_output(list: SmartList) -> Vec<String> {
    let v: Vec<&str> = match list {
        SmartList::Inbox => vec![
            "inbox-1",
            "inbox-2",
            "today-scheduled",
            "today-flag",
            "upcoming-2",
            "upcoming-1",
        ],
        SmartList::Today => vec!["today-scheduled", "today-flag"],
        SmartList::Upcoming => vec!["today-scheduled", "upcoming-2", "proj-task", "upcoming-1"],
        SmartList::Anytime => vec![
            "inbox-1",
            "inbox-2",
            "today-scheduled",
            "proj-task",
            "today-flag",
            "upcoming-2",
            "upcoming-1",
        ],
        SmartList::Someday => vec!["someday-1", "someday-2"],
        SmartList::Logbook => vec![
            "scheduled-today-completed",
            "completed-2",
            "completed-1",
            "trashed-completed",
            "cancelled-1",
        ],
        SmartList::Trash => vec!["trash-2", "trashed-completed", "trash-1"],
    };
    v.into_iter().map(String::from).collect()
}

/// Order: same as SmartList::ALL — Inbox/Today/Upcoming/Anytime/Someday/Logbook/Trash.
pub fn expected_counts() -> [usize; 7] {
    [6, 2, 4, 7, 2, 5, 3]
}

/// Deterministic synthetic dataset — equivalent of TS `makeSynthetic(n)`.
pub fn make_synthetic(n: usize) -> Vec<TodoItem> {
    let mut out = Vec::with_capacity(n);
    for i in 0..n {
        let bucket = i % 10;
        let id = format!("syn-{}", i);
        let mut t = TodoItem {
            sort_order: i as i64,
            created_at: format!("2026-05-{:02}T10:00:00.000Z", (i % 28) + 1),
            ..mk(&id)
        };
        match bucket {
            0..=2 => {} // inbox-ish
            3 => t.is_today = true,
            4 => t.scheduled_date = Some(TODAY_ISO.into()),
            5 => t.scheduled_date = Some("2026-06-01".into()),
            6 => t.is_someday = true,
            7 => {
                t.is_completed = true;
                t.completed_at = Some(t.created_at.clone());
            }
            8 => t.is_trashed = true,
            _ => t.project_id = Some("proj-bench".into()),
        }
        out.push(t);
    }
    out
}
