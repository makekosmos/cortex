import Foundation
import SwiftData

@Model
final class Tag {
    @Attribute(.unique) var id: UUID
    var title: String
    var color: String
    var shortcut: String?

    @Relationship
    var todoItems: [TodoItem]

    init(title: String, color: String = "blue", shortcut: String? = nil) {
        self.id = UUID()
        self.title = title
        self.color = color
        self.shortcut = shortcut
        self.todoItems = []
    }
}
