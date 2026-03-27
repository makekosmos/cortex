import Foundation

// Reusable cached formatters — DateFormatter is expensive to create (~2KB each)
private enum DateFormatters {
    static let weekday: DateFormatter = {
        let f = DateFormatter()
        f.dateFormat = "EEEE"
        return f
    }()

    static let monthDay: DateFormatter = {
        let f = DateFormatter()
        f.dateFormat = "MMM d"
        return f
    }()

    static let monthDayYear: DateFormatter = {
        let f = DateFormatter()
        f.dateFormat = "MMM d, yyyy"
        return f
    }()

}

extension Date {
    var isToday: Bool {
        Calendar.current.isDateInToday(self)
    }

    var isTomorrow: Bool {
        Calendar.current.isDateInTomorrow(self)
    }

    var isYesterday: Bool {
        Calendar.current.isDateInYesterday(self)
    }

    var isPast: Bool {
        self < Calendar.current.startOfDay(for: Date())
    }

    var isFuture: Bool {
        self > Date()
    }

    var startOfDay: Date {
        Calendar.current.startOfDay(for: self)
    }

    var endOfDay: Date {
        Calendar.current.date(bySettingHour: 23, minute: 59, second: 59, of: self) ?? self
    }

    var isThisWeek: Bool {
        Calendar.current.isDate(self, equalTo: Date(), toGranularity: .weekOfYear)
    }

    var isThisMonth: Bool {
        Calendar.current.isDate(self, equalTo: Date(), toGranularity: .month)
    }

    var relativeDisplay: String {
        if isToday { return "Сегодня" }
        if isTomorrow { return "Завтра" }
        if isYesterday { return "Вчера" }

        if isThisWeek {
            return DateFormatters.weekday.string(from: self)
        } else if Calendar.current.isDate(self, equalTo: Date(), toGranularity: .year) {
            return DateFormatters.monthDay.string(from: self)
        } else {
            return DateFormatters.monthDayYear.string(from: self)
        }
    }

    var shortDisplay: String {
        DateFormatters.monthDay.string(from: self)
    }

    func daysUntil(_ other: Date) -> Int {
        Calendar.current.dateComponents([.day], from: startOfDay, to: other.startOfDay).day ?? 0
    }
}
