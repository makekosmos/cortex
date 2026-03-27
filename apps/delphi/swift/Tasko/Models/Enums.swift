import Foundation
import SwiftUI

enum Priority: Int, Codable, CaseIterable, Identifiable {
    case none = 0
    case low = 1
    case medium = 2
    case high = 3

    var id: Int { rawValue }

    var label: String {
        switch self {
        case .none: "Нет"
        case .low: "Низкий"
        case .medium: "Средний"
        case .high: "Высокий"
        }
    }

    var colorName: String {
        switch self {
        case .none: "gray"
        case .low: "green"
        case .medium: "orange"
        case .high: "red"
        }
    }
}

enum ProjectStatus: Int, Codable, CaseIterable {
    case active = 0
    case someday = 1
    case completed = 2

    var label: String {
        switch self {
        case .active: "Активный"
        case .someday: "Потом"
        case .completed: "Завершён"
        }
    }
}

enum Frequency: Int, Codable, CaseIterable, Identifiable {
    case daily = 0
    case weekly = 1
    case monthly = 2
    case yearly = 3

    var id: Int { rawValue }

    var label: String {
        switch self {
        case .daily: "Ежедневно"
        case .weekly: "Еженедельно"
        case .monthly: "Ежемесячно"
        case .yearly: "Ежегодно"
        }
    }
}

enum RecurrenceType: Int, Codable {
    case fixed = 0
    case afterCompletion = 1
}

struct RecurrenceData: Codable, Equatable {
    var frequency: Frequency
    var interval: Int
    var recurrenceType: RecurrenceType
    var daysOfWeek: [Int]?
    var endDate: Date?

    init(
        frequency: Frequency,
        interval: Int = 1,
        recurrenceType: RecurrenceType = .fixed,
        daysOfWeek: [Int]? = nil,
        endDate: Date? = nil
    ) {
        self.frequency = frequency
        self.interval = interval
        self.recurrenceType = recurrenceType
        self.daysOfWeek = daysOfWeek
        self.endDate = endDate
    }

    func nextDate(after date: Date) -> Date? {
        let calendar = Calendar.current
        if let endDate, date >= endDate { return nil }

        switch frequency {
        case .daily:
            return calendar.date(byAdding: .day, value: interval, to: date)
        case .weekly:
            if let days = daysOfWeek, !days.isEmpty {
                let weekday = calendar.component(.weekday, from: date)
                let sorted = days.sorted()
                if let next = sorted.first(where: { $0 > weekday }) {
                    return calendar.date(byAdding: .day, value: next - weekday, to: date)
                }
                if let first = sorted.first {
                    let daysUntil = (7 * interval) - weekday + first
                    return calendar.date(byAdding: .day, value: daysUntil, to: date)
                }
            }
            return calendar.date(byAdding: .weekOfYear, value: interval, to: date)
        case .monthly:
            return calendar.date(byAdding: .month, value: interval, to: date)
        case .yearly:
            return calendar.date(byAdding: .year, value: interval, to: date)
        }
    }
}

enum SmartList: String, CaseIterable, Identifiable {
    case inbox
    case today
    case upcoming
    case anytime
    case someday
    case logbook
    case trash

    var id: String { rawValue }

    var title: String {
        switch self {
        case .inbox: "Входящие"
        case .today: "Сегодня"
        case .upcoming: "Планы"
        case .anytime: "Когда угодно"
        case .someday: "Потом"
        case .logbook: "Журнал"
        case .trash: "Корзина"
        }
    }

    var systemImage: String {
        switch self {
        case .inbox: "tray.fill"
        case .today: "star.fill"
        case .upcoming: "calendar"
        case .anytime: "square.stack.3d.up"
        case .someday: "archivebox"
        case .logbook: "book.closed.fill"
        case .trash: "trash.fill"
        }
    }

    // System colors from Apple HIG
    var iconColor: Color {
        switch self {
        case .inbox: .blue
        case .today: .yellow
        case .upcoming: .red
        case .anytime: .purple
        case .someday: .brown
        case .logbook: .green
        case .trash: .gray
        }
    }

    var keyboardShortcut: KeyEquivalent? {
        switch self {
        case .inbox: "1"
        case .today: "2"
        case .upcoming: "3"
        case .anytime: "4"
        case .someday: "5"
        case .logbook: "6"
        case .trash: nil
        }
    }

    static var topGroup: [SmartList] {
        [.inbox, .today, .upcoming, .anytime, .someday]
    }

    static var bottomGroup: [SmartList] {
        [.logbook, .trash]
    }
}
