import Foundation
import SwiftData
import SwiftUI

@Observable
@MainActor
final class TodoListViewModel {
    var isCreatingNewTodo = false
    var newTodoTitle = ""

    // Multi-selection
    var selectedTodoIDs: Set<UUID> = []

    // Completion animation — track IDs being animated out
    var completingTodoIDs: Set<UUID> = []

    // Tag filter
    var activeTagFilters: Set<UUID> = []
    var isTagFilterAND = false

    // Drag state
    var draggedTodoID: UUID?

    // Inline editing
    var editingTodoID: UUID?

    // Sort
    var sortMode: SortMode = .manual

    enum SortMode: String, CaseIterable {
        case manual = "Вручную"
        case byDate = "По дате"
        case byTitle = "По названию"
        case byCreated = "По дате создания"
    }

    // MARK: - Selection

    var hasMultiSelection: Bool { selectedTodoIDs.count > 1 }

    func selectTodo(_ todo: TodoItem, extend: Bool = false) {
        if extend {
            if selectedTodoIDs.contains(todo.id) {
                selectedTodoIDs.remove(todo.id)
            } else {
                selectedTodoIDs.insert(todo.id)
            }
        } else {
            selectedTodoIDs = [todo.id]
        }
    }

    func extendSelection(to todo: TodoItem, in todos: [TodoItem]) {
        guard let lastID = selectedTodoIDs.first,
              let lastIdx = todos.firstIndex(where: { $0.id == lastID }),
              let newIdx = todos.firstIndex(where: { $0.id == todo.id })
        else {
            selectedTodoIDs = [todo.id]
            return
        }
        let range = min(lastIdx, newIdx)...max(lastIdx, newIdx)
        selectedTodoIDs = Set(todos[range].map(\.id))
    }

    func selectAll(_ todos: [TodoItem]) {
        selectedTodoIDs = Set(todos.map(\.id))
    }

    func clearSelection() {
        selectedTodoIDs.removeAll()
    }

    // MARK: - Single Actions

    func toggleCompletion(_ todo: TodoItem) {
        if todo.isCompleted {
            todo.markIncomplete()
            completingTodoIDs.remove(todo.id)
        } else {
            completingTodoIDs.insert(todo.id)
            todo.markCompleted()
        }
    }

    func cancelTodo(_ todo: TodoItem) {
        todo.markCancelled()
        completingTodoIDs.insert(todo.id)
    }

    func moveToTrash(_ todo: TodoItem) {
        todo.moveToTrash()
        selectedTodoIDs.remove(todo.id)
    }

    func toggleToday(_ todo: TodoItem) {
        todo.isToday.toggle()
        if todo.isToday {
            todo.isSomeday = false
            todo.isEvening = false
        }
    }

    func toggleEvening(_ todo: TodoItem) {
        todo.isEvening.toggle()
        if todo.isEvening {
            todo.isToday = true
            todo.isSomeday = false
        }
    }

    func setSomeday(_ todo: TodoItem) {
        todo.isSomeday = true
        todo.isToday = false
        todo.isEvening = false
        todo.scheduledDate = nil
    }

    func setAnytime(_ todo: TodoItem) {
        todo.isSomeday = false
        todo.scheduledDate = nil
    }

    // MARK: - Batch Actions

    func batchComplete(_ todos: [TodoItem]) {
        for todo in todos where selectedTodoIDs.contains(todo.id) {
            toggleCompletion(todo)
        }
    }

    func batchCancel(_ todos: [TodoItem]) {
        for todo in todos where selectedTodoIDs.contains(todo.id) {
            cancelTodo(todo)
        }
    }

    func batchMoveToTrash(_ todos: [TodoItem]) {
        for todo in todos where selectedTodoIDs.contains(todo.id) {
            moveToTrash(todo)
        }
    }

    func batchSetToday(_ todos: [TodoItem]) {
        for todo in todos where selectedTodoIDs.contains(todo.id) {
            todo.isToday = true
            todo.isSomeday = false
        }
    }

    func batchSetEvening(_ todos: [TodoItem]) {
        for todo in todos where selectedTodoIDs.contains(todo.id) {
            todo.isEvening = true
            todo.isToday = true
            todo.isSomeday = false
        }
    }

    // MARK: - Move

    func moveTodo(_ todo: TodoItem, up: Bool, in todos: inout [TodoItem]) {
        guard let idx = todos.firstIndex(where: { $0.id == todo.id }) else { return }
        let newIdx = up ? max(0, idx - 1) : min(todos.count - 1, idx + 1)
        guard idx != newIdx else { return }
        todos.swapAt(idx, newIdx)
        for (i, t) in todos.enumerated() { t.sortOrder = i }
    }

    // MARK: - Tag Filtering

    func toggleTagFilter(_ tag: Tag) {
        if activeTagFilters.contains(tag.id) {
            activeTagFilters.remove(tag.id)
        } else {
            activeTagFilters.insert(tag.id)
        }
    }

    func clearTagFilters() {
        activeTagFilters.removeAll()
    }

    func matchesTagFilter(_ todo: TodoItem) -> Bool {
        guard !activeTagFilters.isEmpty else { return true }
        let todoTagIDs = Set(todo.tags.map(\.id))
        if isTagFilterAND {
            return activeTagFilters.isSubset(of: todoTagIDs)
        } else {
            return !activeTagFilters.isDisjoint(with: todoTagIDs)
        }
    }

    // MARK: - Creation

    func startCreatingTodo() {
        isCreatingNewTodo = true
        newTodoTitle = ""
    }

    // MARK: - Animation Cleanup

    func removeCompletedFromView(_ todoID: UUID) {
        completingTodoIDs.remove(todoID)
    }
}
