import Foundation

struct NaturalDateParser {
    static func parse(_ input: String, relativeTo base: Date = Date()) -> Date? {
        let text = input.trimmingCharacters(in: .whitespaces).lowercased()
        if text.isEmpty { return nil }

        let calendar = Calendar.current

        // Direct keywords
        switch text {
        case "today", "tod", "сегодня":
            return calendar.startOfDay(for: base)
        case "tomorrow", "tom", "завтра":
            return calendar.date(byAdding: .day, value: 1, to: calendar.startOfDay(for: base))
        case "yesterday", "вчера":
            return calendar.date(byAdding: .day, value: -1, to: calendar.startOfDay(for: base))
        default:
            break
        }

        // Abbreviations: 3d, 2w, 3mo, 1y
        if let result = parseAbbreviation(text, calendar: calendar, base: base) {
            return result
        }

        // "in X days/weeks/months/years"
        if let result = parseInPhrase(text, calendar: calendar, base: base) {
            return result
        }

        // "next monday", "next week", etc.
        if let result = parseNextPhrase(text, calendar: calendar, base: base) {
            return result
        }

        // Day names: "monday", "tuesday", etc.
        if let result = parseDayName(text, calendar: calendar, base: base) {
            return result
        }

        // Russian day names
        if let result = parseRussianDayName(text, calendar: calendar, base: base) {
            return result
        }

        // "в понедельник", "в среду"
        if text.hasPrefix("в ") || text.hasPrefix("во ") {
            let dayPart = text.replacingOccurrences(of: "^(в|во)\\s+", with: "", options: .regularExpression)
            if let result = parseRussianDayName(dayPart, calendar: calendar, base: base) {
                return result
            }
        }

        return nil
    }

    private static func parseAbbreviation(_ text: String, calendar: Calendar, base: Date) -> Date? {
        let pattern = #"^(\d+)\s*(d|д|w|н|mo|мес|m|y|г|л)$"#
        guard let match = text.range(of: pattern, options: .regularExpression) else { return nil }
        let matched = String(text[match])

        let digits = matched.prefix(while: \.isNumber)
        guard let count = Int(digits) else { return nil }
        let unit = String(matched.drop(while: \.isNumber)).trimmingCharacters(in: .whitespaces)

        switch unit {
        case "d", "д":
            return calendar.date(byAdding: .day, value: count, to: calendar.startOfDay(for: base))
        case "w", "н":
            return calendar.date(byAdding: .weekOfYear, value: count, to: calendar.startOfDay(for: base))
        case "mo", "мес", "m":
            return calendar.date(byAdding: .month, value: count, to: calendar.startOfDay(for: base))
        case "y", "г", "л":
            return calendar.date(byAdding: .year, value: count, to: calendar.startOfDay(for: base))
        default:
            return nil
        }
    }

    private static func parseInPhrase(_ text: String, calendar: Calendar, base: Date) -> Date? {
        let pattern = #"^(in|через)\s+(\d+)\s+(day|days|week|weeks|month|months|year|years|день|дня|дней|неделю|недели|недель|месяц|месяца|месяцев|год|года|лет)$"#
        guard let regex = try? NSRegularExpression(pattern: pattern, options: .caseInsensitive),
              let match = regex.firstMatch(in: text, range: NSRange(text.startIndex..., in: text)),
              match.numberOfRanges >= 4
        else { return nil }

        let countRange = Range(match.range(at: 2), in: text)!
        let unitRange = Range(match.range(at: 3), in: text)!
        guard let count = Int(text[countRange]) else { return nil }
        let unit = String(text[unitRange]).lowercased()

        let start = calendar.startOfDay(for: base)
        switch unit {
        case "day", "days", "день", "дня", "дней":
            return calendar.date(byAdding: .day, value: count, to: start)
        case "week", "weeks", "неделю", "недели", "недель":
            return calendar.date(byAdding: .weekOfYear, value: count, to: start)
        case "month", "months", "месяц", "месяца", "месяцев":
            return calendar.date(byAdding: .month, value: count, to: start)
        case "year", "years", "год", "года", "лет":
            return calendar.date(byAdding: .year, value: count, to: start)
        default:
            return nil
        }
    }

    private static func parseNextPhrase(_ text: String, calendar: Calendar, base: Date) -> Date? {
        guard text.hasPrefix("next ") || text.hasPrefix("следующ") else { return nil }
        let remainder = text
            .replacingOccurrences(of: "^(next|следующий|следующая|следующее|следующую|следующем)\\s+", with: "", options: .regularExpression)

        if remainder == "week" || remainder == "неделю" || remainder == "неделя" {
            return calendar.date(byAdding: .weekOfYear, value: 1, to: calendar.startOfDay(for: base))
        }
        if remainder == "month" || remainder == "месяц" {
            return calendar.date(byAdding: .month, value: 1, to: calendar.startOfDay(for: base))
        }

        // "next monday" etc.
        if let dayResult = parseDayName(remainder, calendar: calendar, base: base) {
            // If parseDayName returns this week, push to next week
            if let nextWeek = calendar.date(byAdding: .weekOfYear, value: 1, to: calendar.startOfDay(for: base)) {
                if let result = parseDayName(remainder, calendar: calendar, base: nextWeek) {
                    return result
                }
            }
            return dayResult
        }

        return nil
    }

    private static let englishDays: [String: Int] = [
        "sunday": 1, "sun": 1,
        "monday": 1 + 1, "mon": 2,
        "tuesday": 3, "tue": 3, "tues": 3,
        "wednesday": 4, "wed": 4,
        "thursday": 5, "thu": 5, "thur": 5, "thurs": 5,
        "friday": 6, "fri": 6,
        "saturday": 7, "sat": 7
    ]

    private static func parseDayName(_ text: String, calendar: Calendar, base: Date) -> Date? {
        guard let targetWeekday = englishDays[text] else { return nil }
        let currentWeekday = calendar.component(.weekday, from: base)
        var daysAhead = targetWeekday - currentWeekday
        if daysAhead <= 0 { daysAhead += 7 }
        return calendar.date(byAdding: .day, value: daysAhead, to: calendar.startOfDay(for: base))
    }

    private static let russianDays: [String: Int] = [
        "воскресенье": 1, "вс": 1,
        "понедельник": 2, "пн": 2,
        "вторник": 3, "вт": 3,
        "среда": 4, "среду": 4, "ср": 4,
        "четверг": 5, "чт": 5,
        "пятница": 6, "пятницу": 6, "пт": 6,
        "суббота": 7, "субботу": 7, "сб": 7
    ]

    private static func parseRussianDayName(_ text: String, calendar: Calendar, base: Date) -> Date? {
        guard let targetWeekday = russianDays[text] else { return nil }
        let currentWeekday = calendar.component(.weekday, from: base)
        var daysAhead = targetWeekday - currentWeekday
        if daysAhead <= 0 { daysAhead += 7 }
        return calendar.date(byAdding: .day, value: daysAhead, to: calendar.startOfDay(for: base))
    }
}
