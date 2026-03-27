import SwiftUI

struct DelphiCommands: Commands {
    var body: some Commands {
        CommandGroup(replacing: .newItem) {
            Button("Новая задача") {
                NotificationCenter.default.post(name: .newTodo, object: nil)
            }
            .keyboardShortcut("n", modifiers: .command)

            Button("Новый проект") {
                NotificationCenter.default.post(name: .newProject, object: nil)
            }
            .keyboardShortcut("n", modifiers: [.command, .option])

            Button("Новый заголовок") {
                NotificationCenter.default.post(name: .newHeading, object: nil)
            }
            .keyboardShortcut("n", modifiers: [.command, .shift])
        }

        CommandGroup(after: .pasteboard) {
            Divider()

            Button("Выполнить") {
                NotificationCenter.default.post(name: .completeTodo, object: nil)
            }
            .keyboardShortcut("k", modifiers: .command)

            Button("Отменить") {
                NotificationCenter.default.post(name: .cancelTodo, object: nil)
            }
            .keyboardShortcut("k", modifiers: [.command, .option])

            Button("Дублировать") {
                NotificationCenter.default.post(name: .duplicateTodo, object: nil)
            }
            .keyboardShortcut("d", modifiers: .command)

            Divider()

            Button("Переместить в…") {
                NotificationCenter.default.post(name: .moveTodo, object: nil)
            }
            .keyboardShortcut("m", modifiers: [.command, .shift])
        }

        CommandMenu("Расписание") {
            Button("Когда…") {
                NotificationCenter.default.post(name: .setWhen, object: nil)
            }
            .keyboardShortcut("s", modifiers: .command)

            Button("Начать сегодня") {
                NotificationCenter.default.post(name: .setToday, object: nil)
            }
            .keyboardShortcut("t", modifiers: .command)

            Button("Этим вечером") {
                NotificationCenter.default.post(name: .setEvening, object: nil)
            }
            .keyboardShortcut("e", modifiers: .command)

            Button("Когда угодно") {
                NotificationCenter.default.post(name: .setAnytime, object: nil)
            }
            .keyboardShortcut("r", modifiers: .command)

            Button("Потом") {
                NotificationCenter.default.post(name: .setSomeday, object: nil)
            }
            .keyboardShortcut("o", modifiers: .command)

            Divider()

            Button("Дедлайн…") {
                NotificationCenter.default.post(name: .setDeadline, object: nil)
            }
            .keyboardShortcut("d", modifiers: [.command, .shift])

            Divider()

            Button("Теги…") {
                NotificationCenter.default.post(name: .editTags, object: nil)
            }
            .keyboardShortcut("t", modifiers: [.command, .shift])
        }

        CommandGroup(replacing: .textEditing) {
            Button("Быстрый поиск") {
                NotificationCenter.default.post(name: .quickFind, object: nil)
            }
            .keyboardShortcut("f", modifiers: .command)
        }

        CommandGroup(after: .sidebar) {
            Button("Боковая панель") {
                NotificationCenter.default.post(name: .toggleSidebar, object: nil)
            }
            .keyboardShortcut("/", modifiers: .command)

            Divider()

            Button("Входящие") {
                NotificationCenter.default.post(name: .navigateToList, object: SmartList.inbox)
            }
            .keyboardShortcut("1", modifiers: .command)

            Button("Сегодня") {
                NotificationCenter.default.post(name: .navigateToList, object: SmartList.today)
            }
            .keyboardShortcut("2", modifiers: .command)

            Button("Планы") {
                NotificationCenter.default.post(name: .navigateToList, object: SmartList.upcoming)
            }
            .keyboardShortcut("3", modifiers: .command)

            Button("Когда угодно") {
                NotificationCenter.default.post(name: .navigateToList, object: SmartList.anytime)
            }
            .keyboardShortcut("4", modifiers: .command)

            Button("Потом") {
                NotificationCenter.default.post(name: .navigateToList, object: SmartList.someday)
            }
            .keyboardShortcut("5", modifiers: .command)

            Button("Журнал") {
                NotificationCenter.default.post(name: .navigateToList, object: SmartList.logbook)
            }
            .keyboardShortcut("6", modifiers: .command)
        }
    }
}

// MARK: - Notification Names

extension Notification.Name {
    static let newTodo = Notification.Name("newTodo")
    static let newProject = Notification.Name("newProject")
    static let newHeading = Notification.Name("newHeading")
    static let completeTodo = Notification.Name("completeTodo")
    static let cancelTodo = Notification.Name("cancelTodo")
    static let duplicateTodo = Notification.Name("duplicateTodo")
    static let moveTodo = Notification.Name("moveTodo")
    static let setWhen = Notification.Name("setWhen")
    static let setToday = Notification.Name("setToday")
    static let setEvening = Notification.Name("setEvening")
    static let setAnytime = Notification.Name("setAnytime")
    static let setSomeday = Notification.Name("setSomeday")
    static let setDeadline = Notification.Name("setDeadline")
    static let editTags = Notification.Name("editTags")
    static let toggleSidebar = Notification.Name("toggleSidebar")
    static let navigateToList = Notification.Name("navigateToList")
    static let quickFind = Notification.Name("quickFind")
    static let quickOpenSelected = Notification.Name("quickOpenSelected")
}
