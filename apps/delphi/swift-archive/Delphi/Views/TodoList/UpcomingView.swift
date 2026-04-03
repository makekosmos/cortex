import SwiftUI
import SwiftData

struct UpcomingView: View {
    @Bindable var viewModel: TodoListViewModel
    @Environment(\.modelContext) private var modelContext
    @Environment(ArkSyncClient.self) private var syncClient
    @Query private var allTodos: [TodoItem]
    @State private var expandedTodoID: UUID?
    @State private var cachedSections: [UpcomingSection] = []

    private static let headerDateFormatter: DateFormatter = {
        let f = DateFormatter()
        f.dateFormat = "E, MMM d"
        return f
    }()

    private static let dayNameFormatter: DateFormatter = {
        let f = DateFormatter()
        f.dateFormat = "EEEE"
        return f
    }()

    private static let monthFormatter: DateFormatter = {
        let f = DateFormatter()
        f.dateFormat = "LLLL yyyy"
        f.locale = Locale(identifier: "ru_RU")
        return f
    }()

    private var upcomingTodos: [TodoItem] {
        allTodos.filter {
            $0.scheduledDate != nil && !$0.isCompleted && !$0.isCancelled && !$0.isTrashed && !$0.isSomeday
        }
    }

    private func recomputeSections() {
        cachedSections = computeSections()
    }

    private func computeSections() -> [UpcomingSection] {
        let calendar = Calendar.current
        let today = calendar.startOfDay(for: Date())

        var result: [UpcomingSection] = []

        // Next 7 days individually
        for dayOffset in 1...7 {
            guard let date = calendar.date(byAdding: .day, value: dayOffset, to: today) else { continue }
            let dayTodos = upcomingTodos.filter {
                guard let sched = $0.scheduledDate else { return false }
                return calendar.isDate(sched, inSameDayAs: date)
            }.sorted { $0.sortOrder < $1.sortOrder }

            let title = dayTitle(for: date, offset: dayOffset)
            result.append(UpcomingSection(id: "day-\(dayOffset)", title: title, date: date, todos: dayTodos))
        }

        // Group by week/month beyond 7 days
        guard let weekBoundary = calendar.date(byAdding: .day, value: 8, to: today),
              let monthBoundary = calendar.date(byAdding: .month, value: 1, to: today),
              let yearBoundary = calendar.date(byAdding: .year, value: 1, to: today)
        else { return result }

        for weekOffset in 2...4 {
            guard let weekStart = calendar.date(byAdding: .weekOfYear, value: weekOffset - 1, to: weekBoundary),
                  let weekEnd = calendar.date(byAdding: .weekOfYear, value: 1, to: weekStart)
            else { continue }

            let weekTodos = upcomingTodos.filter {
                guard let sched = $0.scheduledDate else { return false }
                return sched >= weekStart && sched < weekEnd
            }.sorted { ($0.scheduledDate ?? .distantFuture) < ($1.scheduledDate ?? .distantFuture) }

            if !weekTodos.isEmpty {
                let weekNum = calendar.component(.weekOfYear, from: weekStart)
                result.append(UpcomingSection(id: "week-\(weekNum)", title: "Неделя \(weekNum)", date: weekStart, todos: weekTodos))
            }
        }

        var monthCursor = monthBoundary
        while monthCursor < yearBoundary {
            guard let nextMonth = calendar.date(byAdding: .month, value: 1, to: monthCursor) else { break }
            let monthTodos = upcomingTodos.filter {
                guard let sched = $0.scheduledDate else { return false }
                return sched >= monthCursor && sched < nextMonth
            }.sorted { ($0.scheduledDate ?? .distantFuture) < ($1.scheduledDate ?? .distantFuture) }

            if !monthTodos.isEmpty {
                let monthTitle = Self.monthFormatter.string(from: monthCursor).capitalized
                result.append(UpcomingSection(id: "month-\(monthTitle)", title: monthTitle, date: monthCursor, todos: monthTodos))
            }
            monthCursor = nextMonth
        }

        return result
    }

    // MARK: - Body

    var body: some View {
        ZStack {
            VStack(spacing: 0) {
                // Header
                HStack(spacing: 10) {
                    Image(systemName: SmartList.upcoming.systemImage)
                        .font(.system(size: 24, weight: .medium))
                        .foregroundStyle(SmartList.upcoming.iconColor)

                    Text("Планы")
                        .font(.system(size: 26, weight: .bold))
                        .foregroundStyle(.primary)

                    Spacer()
                }
                .padding(.horizontal, 28)
                .padding(.top, 24)
                .padding(.bottom, 12)

                ScrollView {
                    LazyVStack(spacing: 0, pinnedViews: [.sectionHeaders]) {
                        ForEach(cachedSections) { section in
                            Section {
                                if section.todos.isEmpty {
                                    Text("Нет задач")
                                        .font(.system(size: 12))
                                        .foregroundStyle(.tertiary)
                                        .padding(.horizontal, 28)
                                        .padding(.vertical, 8)
                                } else {
                                    ForEach(section.todos) { todo in
                                        TodoRowView(
                                            todo: todo,
                                            isSelected: viewModel.selectedTodoIDs.contains(todo.id),
                                            isExpanded: expandedTodoID == todo.id,
                                            onToggle: {
                                                withAnimation(.spring(response: 0.3, dampingFraction: 0.7)) {
                                                    viewModel.toggleCompletion(todo)
                                                }
                                                syncClient.sendTodoChange(todo, changeType: "update")
                                            },
                                            onSelect: {
                                                viewModel.selectTodo(todo)
                                            },
                                            onToggleExpand: {
                                                withAnimation(.spring(response: 0.25, dampingFraction: 0.8)) {
                                                    expandedTodoID = expandedTodoID == todo.id ? nil : todo.id
                                                }
                                            }
                                        )
                                        .padding(.horizontal, 16)
                                        .contextMenu {
                                            Button("На сегодня") { viewModel.toggleToday(todo); syncClient.sendTodoChange(todo, changeType: "update") }
                                            Button("В корзину", role: .destructive) { viewModel.moveToTrash(todo); syncClient.sendTodoChange(todo, changeType: "update") }
                                        }
                                    }
                                }
                            } header: {
                                sectionHeader(section)
                            }
                        }
                    }
                    .padding(.top, 4)
                }
            }

        }
        .frame(maxWidth: .infinity, maxHeight: .infinity)
        .toolbar(.hidden, for: .automatic)
        .toolbarBackground(.hidden)
        .onAppear { recomputeSections() }
        .onChange(of: allTodos.count) { _, _ in recomputeSections() }
    }

    @ViewBuilder
    private func sectionHeader(_ section: UpcomingSection) -> some View {
        HStack(spacing: 8) {
            Text(section.title)
                .font(.system(size: 14, weight: .bold))
                .foregroundStyle(.primary)

            if !section.todos.isEmpty {
                Text("\(section.todos.count)")
                    .font(.system(size: 12, design: .rounded).weight(.medium))
                    .foregroundStyle(.secondary)
            }

            Spacer()

            Text(Self.headerDateFormatter.string(from: section.date))
                .font(.system(size: 11))
                .foregroundStyle(.tertiary)
        }
        .padding(.horizontal, 28)
        .padding(.vertical, 8)
        .background(.bar)
    }

    private func dayTitle(for date: Date, offset: Int) -> String {
        if Calendar.current.isDateInTomorrow(date) { return "Завтра" }
        return Self.dayNameFormatter.string(from: date).capitalized
    }
}

struct UpcomingSection: Identifiable {
    let id: String
    let title: String
    let date: Date
    let todos: [TodoItem]
}
