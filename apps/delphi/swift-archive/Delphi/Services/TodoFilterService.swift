import Foundation
import SwiftData

/// Centralized filtering logic to avoid duplication between Sidebar counts and TodoListView.
enum TodoFilterService {

    static func count(for list: SmartList, in todos: [TodoItem]) -> Int {
        let calendar = Calendar.current
        switch list {
        case .inbox:
            return todos.count(where: { $0.project == nil && !$0.isSomeday && !$0.isCompleted && !$0.isCancelled && !$0.isTrashed })
        case .today:
            return todos.count(where: { todo in
                !todo.isCompleted && !todo.isCancelled && !todo.isTrashed && (
                    todo.isToday ||
                    (todo.scheduledDate != nil && calendar.isDateInToday(todo.scheduledDate!))
                )
            })
        case .upcoming:
            return todos.count(where: { $0.scheduledDate != nil && !$0.isCompleted && !$0.isCancelled && !$0.isTrashed && !$0.isSomeday })
        case .anytime:
            return todos.count(where: { !$0.isCompleted && !$0.isCancelled && !$0.isSomeday && !$0.isTrashed })
        case .someday:
            return todos.count(where: { $0.isSomeday && !$0.isCompleted && !$0.isCancelled && !$0.isTrashed })
        case .logbook:
            return todos.count(where: { $0.isCompleted || $0.isCancelled })
        case .trash:
            return todos.count(where: \.isTrashed)
        }
    }

    static func filter(for list: SmartList, in todos: [TodoItem]) -> [TodoItem] {
        let calendar = Calendar.current
        switch list {
        case .inbox:
            return todos
                .filter { $0.project == nil && !$0.isSomeday && !$0.isCompleted && !$0.isCancelled && !$0.isTrashed }
                .sorted { $0.createdAt > $1.createdAt }
        case .today:
            return todos
                .filter { todo in
                    !todo.isCompleted && !todo.isCancelled && !todo.isTrashed && (
                        todo.isToday ||
                        (todo.scheduledDate != nil && calendar.isDateInToday(todo.scheduledDate!))
                    )
                }
                .sorted { $0.sortOrder < $1.sortOrder }
        case .upcoming:
            return todos
                .filter { $0.scheduledDate != nil && !$0.isCompleted && !$0.isCancelled && !$0.isTrashed && !$0.isSomeday }
                .sorted { ($0.scheduledDate ?? .distantFuture) < ($1.scheduledDate ?? .distantFuture) }
        case .anytime:
            return todos
                .filter { !$0.isCompleted && !$0.isCancelled && !$0.isSomeday && !$0.isTrashed }
                .sorted { $0.createdAt > $1.createdAt }
        case .someday:
            return todos
                .filter { $0.isSomeday && !$0.isCompleted && !$0.isCancelled && !$0.isTrashed }
                .sorted { $0.createdAt > $1.createdAt }
        case .logbook:
            return todos
                .filter { $0.isCompleted || $0.isCancelled }
                .sorted { ($0.completedAt ?? $0.cancelledAt ?? .distantPast) > ($1.completedAt ?? $1.cancelledAt ?? .distantPast) }
        case .trash:
            return todos
                .filter(\.isTrashed)
                .sorted { $0.createdAt > $1.createdAt }
        }
    }
}
