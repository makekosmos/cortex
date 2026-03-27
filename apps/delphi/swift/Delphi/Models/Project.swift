import Foundation
import SwiftData

@Model
final class Project {
    var id: UUID
    var title: String
    var notes: String?
    var status: ProjectStatus
    var scheduledDate: Date?
    var deadline: Date?
    var sortOrder: Int
    var colorTag: String?
    var createdAt: Date

    @Relationship(deleteRule: .nullify)
    var todoItems: [TodoItem]

    @Relationship(deleteRule: .cascade, inverse: \Heading.project)
    var headings: [Heading]

    @Relationship(inverse: \Area.projects)
    var area: Area?

    init(
        title: String,
        notes: String? = nil,
        status: ProjectStatus = .active,
        area: Area? = nil,
        colorTag: String? = nil
    ) {
        self.id = UUID()
        self.title = title
        self.notes = notes
        self.status = status
        self.scheduledDate = nil
        self.deadline = nil
        self.sortOrder = 0
        self.colorTag = colorTag
        self.createdAt = Date()
        self.todoItems = []
        self.headings = []
        self.area = area
    }

    var completedCount: Int {
        todoItems.filter(\.isCompleted).count
    }

    var totalCount: Int {
        todoItems.filter { !$0.isTrashed }.count
    }

    var progress: Double {
        guard totalCount > 0 else { return 0 }
        return Double(completedCount) / Double(totalCount)
    }
}
