import Foundation
import SwiftData

@Model
final class TodoItem {
    var id: UUID
    var title: String
    var notes: String?
    var priority: Priority
    var scheduledDate: Date?
    var deadline: Date?
    var reminderDate: Date?
    var isToday: Bool = false
    var isEvening: Bool = false
    var isSomeday: Bool = false
    var isCompleted: Bool = false
    var completedAt: Date?
    var isCancelled: Bool = false
    var cancelledAt: Date?
    var isTrashed: Bool = false
    var sortOrder: Int = 0
    var createdAt: Date
    var headingID: UUID?

    @Relationship(deleteRule: .cascade, inverse: \ChecklistItem.todoItem)
    var checklistItems: [ChecklistItem]

    @Relationship(inverse: \Tag.todoItems)
    var tags: [Tag]

    @Relationship(inverse: \Project.todoItems)
    var project: Project?

    @Relationship(inverse: \Area.todoItems)
    var area: Area?

    var recurrenceRule: RecurrenceData?

    init(
        title: String,
        notes: String? = nil,
        priority: Priority = .none,
        scheduledDate: Date? = nil,
        deadline: Date? = nil,
        reminderDate: Date? = nil,
        isToday: Bool = false,
        isEvening: Bool = false,
        isSomeday: Bool = false,
        project: Project? = nil,
        area: Area? = nil,
        headingID: UUID? = nil
    ) {
        self.id = UUID()
        self.title = title
        self.notes = notes
        self.priority = priority
        self.scheduledDate = scheduledDate
        self.deadline = deadline
        self.reminderDate = reminderDate
        self.isToday = isToday
        self.isEvening = isEvening
        self.isSomeday = isSomeday
        self.isCompleted = false
        self.completedAt = nil
        self.isCancelled = false
        self.cancelledAt = nil
        self.isTrashed = false
        self.sortOrder = 0
        self.createdAt = Date()
        self.headingID = headingID
        self.checklistItems = []
        self.tags = []
        self.project = project
        self.area = area
        self.recurrenceRule = nil
    }

    func markCompleted() {
        isCompleted = true
        completedAt = Date()
        isCancelled = false
        cancelledAt = nil
    }

    func markIncomplete() {
        isCompleted = false
        completedAt = nil
        isCancelled = false
        cancelledAt = nil
    }

    func markCancelled() {
        isCancelled = true
        cancelledAt = Date()
        isCompleted = false
        completedAt = nil
    }

    func moveToTrash() {
        isTrashed = true
    }

    func restore() {
        isTrashed = false
    }

    func duplicate() -> TodoItem {
        let copy = TodoItem(
            title: title,
            notes: notes,
            priority: priority,
            scheduledDate: scheduledDate,
            deadline: deadline,
            reminderDate: reminderDate,
            isToday: isToday,
            isEvening: isEvening,
            isSomeday: isSomeday,
            project: project,
            area: area,
            headingID: headingID
        )
        copy.recurrenceRule = recurrenceRule
        return copy
    }

    func createNextRecurrence() -> TodoItem? {
        guard let rule = recurrenceRule else { return nil }
        let baseDate: Date
        switch rule.recurrenceType {
        case .fixed:
            baseDate = scheduledDate ?? Date()
        case .afterCompletion:
            baseDate = completedAt ?? Date()
        }
        guard let nextDate = rule.nextDate(after: baseDate) else { return nil }
        let next = duplicate()
        next.scheduledDate = nextDate
        next.isCompleted = false
        next.completedAt = nil
        next.isToday = false
        next.isEvening = false
        return next
    }
}
