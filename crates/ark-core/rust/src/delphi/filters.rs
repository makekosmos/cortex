//! Port of `extensions/delphi/src/services/filters/todoFilterService.ts`.
//!
//! Pure functions over `&[TodoItem]`. Никакой DB, никакой I/O — для I/O
//! есть ARK operations выше (которые fetch task_obj, deserialize в
//! TodoItem, потом дёргают эти filters).
//!
//! Parity-tested: см. `tests::golden_parity_*` + `.agent/tasks/...`.
//!
//! Critical invariant: behaviour MUST совпадать с TS `filterTodos()` /
//! `countAll()` для одних и тех же inputs. `today_iso` параметр — TS
//! использует `new Date()` (системная дата); Rust получает явно, чтобы
//! тесты были детерминистическими и timezone-independent.

use super::types::{SmartList, TodoItem};

#[inline]
fn is_active(t: &TodoItem) -> bool {
    !t.is_completed && !t.is_cancelled && !t.is_trashed
}

/// Сравнение `YYYY-MM-DD` через date prefix scheduled_date.
/// TS: `new Date(iso).getFullYear/Month/Date === now.*` — учитывает
/// localtz. Rust port: сравниваем ISO-prefix напрямую (today_iso тоже
/// "YYYY-MM-DD"). Это правильно если caller (TS shim) уже нормализовал
/// scheduledDate в local-date string без timezone части.
#[inline]
fn is_date_today(iso: &Option<String>, today_iso: &str) -> bool {
    match iso {
        Some(s) if s.len() >= 10 => &s[..10] == today_iso,
        _ => false,
    }
}

fn predicate(list: SmartList, t: &TodoItem, today_iso: &str) -> bool {
    match list {
        SmartList::Inbox => t.project_id.is_none() && !t.is_someday && is_active(t),
        SmartList::Today => is_active(t) && (t.is_today || is_date_today(&t.scheduled_date, today_iso)),
        SmartList::Upcoming => t.scheduled_date.is_some() && is_active(t) && !t.is_someday,
        SmartList::Anytime => is_active(t) && !t.is_someday,
        SmartList::Someday => t.is_someday && !t.is_completed && !t.is_cancelled && !t.is_trashed,
        SmartList::Logbook => t.is_completed || t.is_cancelled,
        SmartList::Trash => t.is_trashed,
    }
}

/// Стабильная sort comparator — match TS Array.toSorted (stable since ES2019).
fn cmp_for(list: SmartList, a: &TodoItem, b: &TodoItem) -> std::cmp::Ordering {
    use std::cmp::Ordering;
    match list {
        // sortOrder asc
        SmartList::Inbox | SmartList::Today | SmartList::Anytime | SmartList::Someday => {
            a.sort_order.cmp(&b.sort_order)
        }
        // scheduledDate asc (null → "￿" по TS — поставим last).
        SmartList::Upcoming => match (&a.scheduled_date, &b.scheduled_date) {
            (Some(x), Some(y)) => x.cmp(y),
            (Some(_), None) => Ordering::Less,
            (None, Some(_)) => Ordering::Greater,
            (None, None) => Ordering::Equal,
        },
        // (completedAt || cancelledAt) desc
        SmartList::Logbook => {
            let da = a.completed_at.as_deref().or(a.cancelled_at.as_deref()).unwrap_or("");
            let db = b.completed_at.as_deref().or(b.cancelled_at.as_deref()).unwrap_or("");
            db.cmp(da)
        }
        // createdAt desc
        SmartList::Trash => b.created_at.cmp(&a.created_at),
    }
}

/// Equivalent to TS `filterTodos(list, todos)`.
pub fn filter_todos(list: SmartList, todos: &[TodoItem], today_iso: &str) -> Vec<TodoItem> {
    let mut out: Vec<TodoItem> = todos
        .iter()
        .filter(|t| predicate(list, t, today_iso))
        .cloned()
        .collect();
    out.sort_by(|a, b| cmp_for(list, a, b));
    out
}

/// Equivalent to TS `countTodos(list, todos)`.
pub fn count_todos(list: SmartList, todos: &[TodoItem], today_iso: &str) -> usize {
    todos.iter().filter(|t| predicate(list, t, today_iso)).count()
}

/// Equivalent to TS `countAll(todos)`. Single pass, returns one count per list.
pub fn count_all(todos: &[TodoItem], today_iso: &str) -> [(SmartList, usize); 7] {
    let mut counts = [
        (SmartList::Inbox, 0usize),
        (SmartList::Today, 0),
        (SmartList::Upcoming, 0),
        (SmartList::Anytime, 0),
        (SmartList::Someday, 0),
        (SmartList::Logbook, 0),
        (SmartList::Trash, 0),
    ];
    for t in todos {
        for entry in counts.iter_mut() {
            if predicate(entry.0, t, today_iso) {
                entry.1 += 1;
            }
        }
    }
    counts
}

#[cfg(any(test, feature = "bench-fixtures"))]
#[path = "filters_fixtures.rs"]
pub mod fixtures;

#[cfg(test)]
mod tests {
    use super::*;
    use super::fixtures::{expected_filter_output, expected_counts, fixtures, TODAY_ISO};

    #[test]
    fn golden_parity_filter_per_list() {
        let todos = fixtures();
        for list in SmartList::ALL {
            let actual: Vec<String> = filter_todos(list, &todos, TODAY_ISO)
                .into_iter()
                .map(|t| t.id)
                .collect();
            let expected = expected_filter_output(list);
            assert_eq!(actual, expected, "mismatch for {:?}", list);
        }
    }

    #[test]
    fn golden_parity_count_all() {
        let todos = fixtures();
        let counts = count_all(&todos, TODAY_ISO);
        let expected = expected_counts();
        for (list, count) in counts {
            assert_eq!(count, expected[list as usize], "count mismatch {:?}", list);
        }
    }

    #[test]
    fn empty_input() {
        for list in SmartList::ALL {
            assert!(filter_todos(list, &[], TODAY_ISO).is_empty());
            assert_eq!(count_todos(list, &[], TODAY_ISO), 0);
        }
    }
}
