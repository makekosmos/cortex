import SwiftUI
import SwiftData
import Combine

struct QuickOpenView: View {
    @Environment(\.modelContext) private var modelContext
    @Query private var todos: [TodoItem]
    @Query private var projects: [Project]
    @Query private var tags: [Tag]

    @State private var searchText = ""
    @State private var debouncedQuery = ""
    @State private var selectedIndex = 0
    @FocusState private var isSearchFocused: Bool

    let onDismiss: () -> Void

    private var results: [QuickOpenResult] {
        guard !debouncedQuery.isEmpty else { return [] }
        let query = debouncedQuery

        var items: [QuickOpenResult] = []

        // Задачи — lemma search по title + notes, ранжирование по score
        let scoredTodos = todos
            .filter { !$0.isTrashed }
            .compactMap { todo -> (TodoItem, Double)? in
                let titleScore = LemmaSearchService.score(text: todo.title, query: query)
                let notesScore = todo.notes.map { LemmaSearchService.score(text: $0, query: query) } ?? 0
                let best = max(titleScore, notesScore * 0.8) // notes чуть менее приоритетны
                guard best > 0 else { return nil }
                return (todo, best)
            }
            .sorted { $0.1 > $1.1 }
            .prefix(5)

        items += scoredTodos.map { .todo($0.0) }

        // Проекты
        items += projects
            .filter { LemmaSearchService.score(text: $0.title, query: query) > 0 }
            .prefix(3)
            .map { .project($0) }

        // Теги
        items += tags
            .filter { LemmaSearchService.score(text: $0.title, query: query) > 0 }
            .prefix(3)
            .map { .tag($0) }

        return items
    }

    var body: some View {
        VStack(spacing: 0) {
            // Search field
            HStack(spacing: 10) {
                Image(systemName: "magnifyingglass")
                    .foregroundStyle(.secondary)
                    .font(.system(size: 16))

                TextField("Поиск задач, проектов, тегов…", text: $searchText)
                    .textFieldStyle(.plain)
                    .font(.system(size: 16))
                    .focused($isSearchFocused)
                    .onSubmit { activateSelected() }
            }
            .padding(14)

            // Results
            if !results.isEmpty {
                Divider().opacity(0.5)

                ScrollView {
                    LazyVStack(spacing: 0) {
                        ForEach(Array(results.enumerated()), id: \.element.id) { index, result in
                            QuickOpenResultRow(
                                result: result,
                                isSelected: index == selectedIndex
                            )
                            .contentShape(Rectangle())
                            .onTapGesture {
                                selectedIndex = index
                                activateSelected()
                            }
                        }
                    }
                    .padding(.vertical, 4)
                }
                .frame(maxHeight: 280)
            } else if !debouncedQuery.isEmpty {
                Text("Ничего не найдено")
                    .font(.system(size: 13))
                    .foregroundStyle(.secondary)
                    .padding(14)
            }
        }
        .frame(width: 480)
        .background(.ultraThickMaterial, in: RoundedRectangle(cornerRadius: 10))
        .shadow(color: .black.opacity(0.3), radius: 30, y: 8)
        .onAppear {
            DispatchQueue.main.asyncAfter(deadline: .now() + 0.05) {
                isSearchFocused = true
            }
        }
        .onKeyPress(.upArrow) {
            if !results.isEmpty {
                selectedIndex = max(0, selectedIndex - 1)
            }
            return .handled
        }
        .onKeyPress(.downArrow) {
            if !results.isEmpty {
                selectedIndex = min(results.count - 1, selectedIndex + 1)
            }
            return .handled
        }
        .onKeyPress(.escape) {
            onDismiss()
            return .handled
        }
        // Debounce 300ms
        .onChange(of: searchText) { _, newValue in
            selectedIndex = 0
        }
        .task(id: searchText) {
            try? await Task.sleep(for: .milliseconds(300))
            if !Task.isCancelled {
                debouncedQuery = searchText
            }
        }
    }

    private func activateSelected() {
        guard selectedIndex < results.count else { return }
        let result = results[selectedIndex]
        NotificationCenter.default.post(name: .quickOpenSelected, object: result)
        onDismiss()
    }
}

// MARK: - Result types

enum QuickOpenResult: Identifiable {
    case todo(TodoItem)
    case project(Project)
    case tag(Tag)

    var id: String {
        switch self {
        case .todo(let t): "todo-\(t.id)"
        case .project(let p): "project-\(p.id)"
        case .tag(let t): "tag-\(t.id)"
        }
    }
}

struct QuickOpenResultRow: View {
    let result: QuickOpenResult
    let isSelected: Bool

    var body: some View {
        HStack(spacing: 10) {
            Image(systemName: iconName)
                .foregroundStyle(iconColor)
                .frame(width: 20)

            VStack(alignment: .leading, spacing: 1) {
                Text(title)
                    .font(.system(size: 13))
                    .lineLimit(1)
                Text(subtitle)
                    .font(.system(size: 11))
                    .foregroundStyle(.secondary)
            }

            Spacer()
        }
        .padding(.horizontal, 14)
        .padding(.vertical, 7)
        .background(isSelected ? Color.accentColor.opacity(0.1) : .clear)
    }

    private var iconName: String {
        switch result {
        case .todo: "checkmark.circle"
        case .project: "folder"
        case .tag: "tag"
        }
    }

    private var iconColor: Color {
        switch result {
        case .todo: .blue
        case .project: .purple
        case .tag: .orange
        }
    }

    private var title: String {
        switch result {
        case .todo(let t): t.title
        case .project(let p): p.title
        case .tag(let t): t.title
        }
    }

    private var subtitle: String {
        switch result {
        case .todo(let t):
            if t.isCompleted { return "Завершена" }
            if let project = t.project { return project.title }
            return "Входящие"
        case .project(let p): return "\(p.totalCount) задач"
        case .tag(let t): return "\(t.todoItems.count) задач"
        }
    }
}

// quickOpenSelected notification defined in DelphiCommands.swift
