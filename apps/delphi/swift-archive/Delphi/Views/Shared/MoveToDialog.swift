import SwiftUI
import SwiftData

struct MoveToDialog: View {
    let todoIDs: Set<UUID>
    let onDismiss: () -> Void

    @Environment(\.modelContext) private var modelContext
    @Environment(ArkSyncClient.self) private var syncClient
    @Query(sort: \Project.sortOrder) private var projects: [Project]
    @Query(sort: \Area.sortOrder) private var areas: [Area]
    @Query private var allTodos: [TodoItem]
    @Query private var allHeadings: [Heading]

    @State private var searchText = ""
    @State private var selectedIndex = 0

    var destinations: [MoveDestination] {
        var results: [MoveDestination] = []
        let query = searchText.lowercased()

        // Inbox (no project)
        if query.isEmpty || "входящие".contains(query) || "inbox".contains(query) {
            results.append(.inbox)
        }

        // Projects
        for project in projects where project.status == .active {
            if query.isEmpty || project.title.lowercased().contains(query) {
                results.append(.project(project))

                // Headings within project
                for heading in project.headings.sorted(by: { $0.sortOrder < $1.sortOrder }) {
                    if query.isEmpty || heading.title.lowercased().contains(query) {
                        results.append(.heading(heading, project))
                    }
                }
            }
        }

        // Areas
        for area in areas {
            if query.isEmpty || area.title.lowercased().contains(query) {
                results.append(.area(area))
            }
        }

        return results
    }

    var body: some View {
        VStack(spacing: 0) {
            HStack(spacing: 10) {
                Image(systemName: "arrow.right.square")
                    .foregroundStyle(.secondary)
                    .font(.title3)

                TextField("Переместить в…", text: $searchText)
                    .textFieldStyle(.plain)
                    .font(.title3)
                    .onSubmit { activateSelected() }
            }
            .padding(16)

            Divider()

            ScrollView {
                LazyVStack(spacing: 0) {
                    ForEach(Array(destinations.enumerated()), id: \.element.id) { index, dest in
                        HStack(spacing: 10) {
                            Image(systemName: dest.icon)
                                .foregroundStyle(dest.color)
                                .frame(width: 24)

                            VStack(alignment: .leading, spacing: 1) {
                                Text(dest.title)
                                    .font(.body)
                                    .lineLimit(1)
                                if let subtitle = dest.subtitle {
                                    Text(subtitle)
                                        .font(.caption)
                                        .foregroundStyle(.secondary)
                                }
                            }

                            Spacer()
                        }
                        .padding(.horizontal, 16)
                        .padding(.vertical, 8)
                        .background(index == selectedIndex ? Color.accentColor.opacity(0.1) : .clear)
                        .contentShape(Rectangle())
                        .onTapGesture {
                            selectedIndex = index
                            activateSelected()
                        }
                    }
                }
                .padding(.vertical, 4)
            }
            .frame(maxHeight: 300)
        }
        .frame(width: 400)
        .background(.ultraThinMaterial)
        .clipShape(RoundedRectangle(cornerRadius: 12))
        .onKeyPress(.upArrow) {
            selectedIndex = max(0, selectedIndex - 1)
            return .handled
        }
        .onKeyPress(.downArrow) {
            selectedIndex = min(destinations.count - 1, selectedIndex + 1)
            return .handled
        }
        .onExitCommand { onDismiss() }
        .onChange(of: searchText) { _, _ in selectedIndex = 0 }
    }

    private func activateSelected() {
        guard selectedIndex < destinations.count else { return }
        let dest = destinations[selectedIndex]
        let todos = allTodos.filter { todoIDs.contains($0.id) }

        for todo in todos {
            switch dest {
            case .inbox:
                todo.project = nil
                todo.area = nil
                todo.headingID = nil
            case .project(let project):
                todo.project = project
                todo.headingID = nil
            case .heading(let heading, let project):
                todo.project = project
                todo.headingID = heading.id
            case .area(let area):
                todo.area = area
                todo.project = nil
                todo.headingID = nil
            }
            syncClient.sendTodoChange(todo, changeType: "update")
        }

        onDismiss()
    }
}

enum MoveDestination: Identifiable {
    case inbox
    case project(Project)
    case heading(Heading, Project)
    case area(Area)

    var id: String {
        switch self {
        case .inbox: "inbox"
        case .project(let p): "project-\(p.id)"
        case .heading(let h, _): "heading-\(h.id)"
        case .area(let a): "area-\(a.id)"
        }
    }

    var title: String {
        switch self {
        case .inbox: "Входящие"
        case .project(let p): p.title
        case .heading(let h, _): h.title
        case .area(let a): a.title
        }
    }

    var subtitle: String? {
        switch self {
        case .heading(_, let p): "в \(p.title)"
        default: nil
        }
    }

    var icon: String {
        switch self {
        case .inbox: "tray.fill"
        case .project: "folder"
        case .heading: "text.alignleft"
        case .area: "square.stack.fill"
        }
    }

    var color: Color {
        switch self {
        case .inbox: SmartList.inbox.iconColor
        case .project: .blue
        case .heading: .secondary
        case .area: .purple
        }
    }
}
