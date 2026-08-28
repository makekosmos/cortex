pub fn expected_filter_output(list: SmartList) -> Vec<String> {
    let ids: &[&str] = match list {
        SmartList::Inbox => &[
            "inbox-1",
            "inbox-2",
            "today-scheduled",
            "today-flag",
            "upcoming-2",
            "upcoming-1",
        ],
        SmartList::Today => &["today-scheduled", "today-flag"],
        SmartList::Upcoming => &["today-scheduled", "upcoming-2", "proj-task", "upcoming-1"],
        SmartList::Anytime => &[
            "inbox-1",
            "inbox-2",
            "today-scheduled",
            "proj-task",
            "today-flag",
            "upcoming-2",
            "upcoming-1",
        ],
        SmartList::Someday => &["someday-1", "someday-2"],
        SmartList::Logbook => &[
            "scheduled-today-completed",
            "completed-2",
            "completed-1",
            "trashed-completed",
            "cancelled-1",
        ],
        SmartList::Trash => &["trash-2", "trashed-completed", "trash-1"],
    };
    ids.iter().map(|id| (*id).into()).collect()
}

pub fn expected_counts() -> [usize; 7] {
    [6, 2, 4, 7, 2, 5, 3]
}
/// Repeats the parity corpus for benchmark-sized, realistic task lists.
pub fn make_synthetic(count: usize) -> Vec<TodoItem> {
    fixtures().into_iter().cycle().take(count).collect()
}
