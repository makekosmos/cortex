import Foundation
import SwiftData

@Model
final class Area {
    @Attribute(.unique) var id: UUID
    var title: String
    var sortOrder: Int
    var isVisible: Bool

    @Relationship(deleteRule: .nullify)
    var projects: [Project]

    @Relationship(deleteRule: .nullify)
    var todoItems: [TodoItem]

    init(title: String) {
        self.id = UUID()
        self.title = title
        self.sortOrder = 0
        self.isVisible = true
        self.projects = []
        self.todoItems = []
    }
}
