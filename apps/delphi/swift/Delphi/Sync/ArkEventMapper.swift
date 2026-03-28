import Foundation
import SwiftData

/// Maps between SwiftData models and Ark sync event JSON.
enum ArkEventMapper {

    // MARK: - ISO 8601 formatter

    private static let iso8601: ISO8601DateFormatter = {
        let f = ISO8601DateFormatter()
        f.formatOptions = [.withInternetDateTime, .withFractionalSeconds]
        return f
    }()

    // MARK: - TodoItem -> Ark event

    static func todoToArkEvent(_ todo: TodoItem, changeType: String = "create") -> [String: Any] {
        var data: [String: Any] = [
            "title": todo.title,
            "priority": todo.priority.rawValue,
            "isCompleted": todo.isCompleted,
            "isCancelled": todo.isCancelled,
            "isTrashed": todo.isTrashed,
            "isToday": todo.isToday,
            "isEvening": todo.isEvening,
            "isSomeday": todo.isSomeday,
            "sortOrder": todo.sortOrder,
            "createdAt": iso8601.string(from: todo.createdAt),
        ]

        if let notes = todo.notes { data["notes"] = notes }
        if let scheduledDate = todo.scheduledDate { data["scheduledDate"] = iso8601.string(from: scheduledDate) }
        if let deadline = todo.deadline { data["deadline"] = iso8601.string(from: deadline) }
        if let reminderDate = todo.reminderDate { data["reminderDate"] = iso8601.string(from: reminderDate) }
        if let completedAt = todo.completedAt { data["completedAt"] = iso8601.string(from: completedAt) }
        if let cancelledAt = todo.cancelledAt { data["cancelledAt"] = iso8601.string(from: cancelledAt) }
        if let headingID = todo.headingID { data["headingID"] = headingID.uuidString }
        if let projectId = todo.project?.id { data["projectId"] = projectId.uuidString }
        if let areaId = todo.area?.id { data["areaId"] = areaId.uuidString }
        if !todo.tags.isEmpty { data["tagIds"] = todo.tags.map { $0.id.uuidString } }

        if let rule = todo.recurrenceRule {
            data["recurrenceRule"] = [
                "frequency": rule.frequency.rawValue,
                "interval": rule.interval,
                "recurrenceType": rule.recurrenceType.rawValue,
                "daysOfWeek": rule.daysOfWeek as Any,
                "endDate": rule.endDate.map { iso8601.string(from: $0) } as Any,
            ] as [String: Any]
        }

        return [
            "event_type": "task",
            "category": "productivity",
            "source": "delphi",
            "source_id": todo.id.uuidString,
            "summary": todo.title,
            "data": data,
            "change_type": changeType,
        ]
    }

    // MARK: - Project -> Ark event

    static func projectToArkEvent(_ project: Project, changeType: String = "create") -> [String: Any] {
        var data: [String: Any] = [
            "title": project.title,
            "status": project.status.rawValue,
            "sortOrder": project.sortOrder,
            "createdAt": iso8601.string(from: project.createdAt),
        ]

        if let notes = project.notes { data["notes"] = notes }
        if let scheduledDate = project.scheduledDate { data["scheduledDate"] = iso8601.string(from: scheduledDate) }
        if let deadline = project.deadline { data["deadline"] = iso8601.string(from: deadline) }
        if let colorTag = project.colorTag { data["colorTag"] = colorTag }
        if let areaId = project.area?.id { data["areaId"] = areaId.uuidString }

        return [
            "event_type": "project",
            "category": "productivity",
            "source": "delphi",
            "source_id": project.id.uuidString,
            "summary": project.title,
            "data": data,
            "change_type": changeType,
        ]
    }

    // MARK: - Area -> Ark event

    static func areaToArkEvent(_ area: Area, changeType: String = "create") -> [String: Any] {
        let data: [String: Any] = [
            "title": area.title,
            "sortOrder": area.sortOrder,
            "isVisible": area.isVisible,
        ]

        return [
            "event_type": "area",
            "category": "productivity",
            "source": "delphi",
            "source_id": area.id.uuidString,
            "summary": area.title,
            "data": data,
            "change_type": changeType,
        ]
    }

    // MARK: - Tag -> Ark event

    static func tagToArkEvent(_ tag: Tag, changeType: String = "create") -> [String: Any] {
        var data: [String: Any] = [
            "title": tag.title,
            "color": tag.color,
        ]
        if let shortcut = tag.shortcut { data["shortcut"] = shortcut }

        return [
            "event_type": "tag",
            "category": "productivity",
            "source": "delphi",
            "source_id": tag.id.uuidString,
            "summary": tag.title,
            "data": data,
            "change_type": changeType,
        ]
    }

    // MARK: - Ark event -> TodoItem

    static func arkEventToTodo(_ event: [String: Any], context: ModelContext) -> TodoItem? {
        guard let eventData = event["data"] as? [String: Any],
              let title = eventData["title"] as? String
        else { return nil }

        // Swift sends source_id at top level; TS sends it at top level too,
        // but also puts id inside the inner data dict as fallback.
        guard let sourceId = event["source_id"] as? String
                ?? eventData["id"] as? String
        else { return nil }

        // Look up existing by source_id
        let existingId = UUID(uuidString: sourceId)
        if let existingId {
            let descriptor = FetchDescriptor<TodoItem>(predicate: #Predicate { $0.id == existingId })
            if let existing = try? context.fetch(descriptor).first {
                applyTodoData(eventData, to: existing, context: context)
                return existing
            }
        }

        // Create new
        let todo = TodoItem(title: title)
        if let existingId { todo.id = existingId }
        applyTodoData(eventData, to: todo, context: context)
        context.insert(todo)
        return todo
    }

    private static func applyTodoData(_ data: [String: Any], to todo: TodoItem, context: ModelContext) {
        if let title = data["title"] as? String { todo.title = title }
        // Swift uses "notes", TS uses "description"
        if let notes = data["notes"] as? String ?? data["description"] as? String { todo.notes = notes }
        if let priority = data["priority"] as? Int, let p = Priority(rawValue: priority) { todo.priority = p }
        // Swift uses "isCompleted", TS uses "completed"
        if let v = data["isCompleted"] as? Bool ?? data["completed"] as? Bool { todo.isCompleted = v }
        if let v = data["isCancelled"] as? Bool { todo.isCancelled = v }
        if let v = data["isTrashed"] as? Bool { todo.isTrashed = v }
        if let v = data["isToday"] as? Bool { todo.isToday = v }
        if let v = data["isEvening"] as? Bool { todo.isEvening = v }
        if let v = data["isSomeday"] as? Bool { todo.isSomeday = v }
        if let v = data["sortOrder"] as? Int { todo.sortOrder = v }

        if let s = data["scheduledDate"] as? String { todo.scheduledDate = iso8601.date(from: s) }
        if let s = data["deadline"] as? String { todo.deadline = iso8601.date(from: s) }
        if let s = data["reminderDate"] as? String { todo.reminderDate = iso8601.date(from: s) }
        if let s = data["completedAt"] as? String { todo.completedAt = iso8601.date(from: s) }
        if let s = data["cancelledAt"] as? String { todo.cancelledAt = iso8601.date(from: s) }
        if let s = data["headingID"] as? String { todo.headingID = UUID(uuidString: s) }

        // Resolve project relationship
        if let projectIdStr = data["projectId"] as? String, let projectId = UUID(uuidString: projectIdStr) {
            let descriptor = FetchDescriptor<Project>(predicate: #Predicate { $0.id == projectId })
            todo.project = try? context.fetch(descriptor).first
        }

        // Resolve area relationship
        if let areaIdStr = data["areaId"] as? String, let areaId = UUID(uuidString: areaIdStr) {
            let descriptor = FetchDescriptor<Area>(predicate: #Predicate { $0.id == areaId })
            todo.area = try? context.fetch(descriptor).first
        }

        // Resolve tags
        if let tagIdStrs = data["tagIds"] as? [String] {
            let tagIds = tagIdStrs.compactMap { UUID(uuidString: $0) }
            var tags: [Tag] = []
            for tagId in tagIds {
                let descriptor = FetchDescriptor<Tag>(predicate: #Predicate { $0.id == tagId })
                if let tag = try? context.fetch(descriptor).first {
                    tags.append(tag)
                }
            }
            todo.tags = tags
        }

        // Recurrence rule
        if let ruleData = data["recurrenceRule"] as? [String: Any],
           let freqRaw = ruleData["frequency"] as? Int,
           let freq = Frequency(rawValue: freqRaw),
           let interval = ruleData["interval"] as? Int,
           let typeRaw = ruleData["recurrenceType"] as? Int,
           let recType = RecurrenceType(rawValue: typeRaw)
        {
            var endDate: Date?
            if let s = ruleData["endDate"] as? String { endDate = iso8601.date(from: s) }
            let daysOfWeek = ruleData["daysOfWeek"] as? [Int]
            todo.recurrenceRule = RecurrenceData(
                frequency: freq, interval: interval,
                recurrenceType: recType, daysOfWeek: daysOfWeek, endDate: endDate
            )
        }
    }

    // MARK: - Ark event -> Project

    static func arkEventToProject(_ event: [String: Any], context: ModelContext) -> Project? {
        guard let eventData = event["data"] as? [String: Any],
              let title = eventData["title"] as? String,
              let sourceId = event["source_id"] as? String
        else { return nil }

        let existingId = UUID(uuidString: sourceId)
        if let existingId {
            let descriptor = FetchDescriptor<Project>(predicate: #Predicate { $0.id == existingId })
            if let existing = try? context.fetch(descriptor).first {
                applyProjectData(eventData, to: existing, context: context)
                return existing
            }
        }

        let project = Project(title: title)
        if let existingId { project.id = existingId }
        applyProjectData(eventData, to: project, context: context)
        context.insert(project)
        return project
    }

    private static func applyProjectData(_ data: [String: Any], to project: Project, context: ModelContext) {
        if let title = data["title"] as? String { project.title = title }
        if let notes = data["notes"] as? String { project.notes = notes }
        if let statusRaw = data["status"] as? Int, let s = ProjectStatus(rawValue: statusRaw) { project.status = s }
        if let v = data["sortOrder"] as? Int { project.sortOrder = v }
        if let v = data["colorTag"] as? String { project.colorTag = v }
        if let s = data["scheduledDate"] as? String { project.scheduledDate = iso8601.date(from: s) }
        if let s = data["deadline"] as? String { project.deadline = iso8601.date(from: s) }

        if let areaIdStr = data["areaId"] as? String, let areaId = UUID(uuidString: areaIdStr) {
            let descriptor = FetchDescriptor<Area>(predicate: #Predicate { $0.id == areaId })
            project.area = try? context.fetch(descriptor).first
        }
    }

    // MARK: - Ark event -> Area

    static func arkEventToArea(_ event: [String: Any], context: ModelContext) -> Area? {
        guard let eventData = event["data"] as? [String: Any],
              let title = eventData["title"] as? String,
              let sourceId = event["source_id"] as? String
        else { return nil }

        let existingId = UUID(uuidString: sourceId)
        if let existingId {
            let descriptor = FetchDescriptor<Area>(predicate: #Predicate { $0.id == existingId })
            if let existing = try? context.fetch(descriptor).first {
                if let v = eventData["sortOrder"] as? Int { existing.sortOrder = v }
                if let v = eventData["isVisible"] as? Bool { existing.isVisible = v }
                if let t = eventData["title"] as? String { existing.title = t }
                return existing
            }
        }

        let area = Area(title: title)
        if let existingId { area.id = existingId }
        if let v = eventData["sortOrder"] as? Int { area.sortOrder = v }
        if let v = eventData["isVisible"] as? Bool { area.isVisible = v }
        context.insert(area)
        return area
    }

    // MARK: - Ark event -> Tag

    static func arkEventToTag(_ event: [String: Any], context: ModelContext) -> Tag? {
        guard let eventData = event["data"] as? [String: Any],
              let title = eventData["title"] as? String,
              let sourceId = event["source_id"] as? String
        else { return nil }

        let existingId = UUID(uuidString: sourceId)
        if let existingId {
            let descriptor = FetchDescriptor<Tag>(predicate: #Predicate { $0.id == existingId })
            if let existing = try? context.fetch(descriptor).first {
                if let t = eventData["title"] as? String { existing.title = t }
                if let c = eventData["color"] as? String { existing.color = c }
                existing.shortcut = eventData["shortcut"] as? String
                return existing
            }
        }

        let color = eventData["color"] as? String ?? "blue"
        let tag = Tag(title: title, color: color, shortcut: eventData["shortcut"] as? String)
        if let existingId { tag.id = existingId }
        context.insert(tag)
        return tag
    }
}
