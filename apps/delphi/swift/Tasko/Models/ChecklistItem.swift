import Foundation
import SwiftData

@Model
final class ChecklistItem {
    var id: UUID
    var title: String
    var isCompleted: Bool
    var sortOrder: Int

    @Relationship
    var todoItem: TodoItem?

    init(title: String, todoItem: TodoItem? = nil) {
        self.id = UUID()
        self.title = title
        self.isCompleted = false
        self.sortOrder = 0
        self.todoItem = todoItem
    }
}
