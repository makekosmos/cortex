import SwiftUI
import SwiftData

struct SidebarView: View {
    @Bindable var viewModel: SidebarViewModel
    @Environment(\.modelContext) private var modelContext
    @Query(sort: \Area.sortOrder) private var areas: [Area]
    @Query(sort: \Project.sortOrder) private var allProjects: [Project]
    @Query private var allTodos: [TodoItem]

    private var projects: [Project] {
        allProjects.filter { $0.status == .active }
    }

    private var standaloneProjects: [Project] {
        projects.filter { $0.area == nil }
    }

    var body: some View {
        List {
            // Top spacer to align with content header
            Section { }
                .listSectionSeparator(.hidden)
                .listRowInsets(EdgeInsets(top: 16, leading: 0, bottom: 0, trailing: 0))

            // Top smart lists
            Section {
                ForEach(SmartList.topGroup, id: \.self) { list in
                    smartListRow(list)
                }
            }
            .listSectionSeparator(.visible, edges: .bottom)

            // Bottom smart lists
            Section {
                ForEach(SmartList.bottomGroup, id: \.self) { list in
                    smartListRow(list)
                }
            }
            .listSectionSeparator(.visible, edges: .bottom)

            // Standalone projects
            if !standaloneProjects.isEmpty {
                Section {
                    ForEach(standaloneProjects) { project in
                        projectRow(project)
                    }
                }
                .listSectionSeparator(.hidden)
            }

            // Areas with projects
            ForEach(areas) { area in
                Section {
                    areaRow(area)

                    if !viewModel.isAreaCollapsed(area) {
                        ForEach(area.projects.filter { $0.status == .active }.sorted { $0.sortOrder < $1.sortOrder }) { project in
                            projectRow(project)
                                .padding(.leading, 8)
                        }
                    }
                }
                .listSectionSeparator(.hidden)
            }
        }
        .listStyle(.sidebar)
        .safeAreaInset(edge: .bottom, spacing: 0) {
            bottomBar
        }
        .sheet(isPresented: $viewModel.isShowingNewProject) {
            NewProjectSheet(viewModel: viewModel)
        }
        // Drop targets for drag-to-sidebar
        .dropDestination(for: String.self) { items, _ in
            // Handle dropping todos onto sidebar (move to inbox)
            return false
        }
    }

    // MARK: - Smart List Row

    @ViewBuilder
    private func smartListRow(_ list: SmartList) -> some View {
        SidebarSmartListRow(
            list: list,
            isSelected: viewModel.selectedSmartList == list,
            count: countFor(list),
            onSelect: {
                withAnimation(.easeInOut(duration: 0.15)) {
                    viewModel.selectSmartList(list)
                }
            }
        )
    }

    // MARK: - Project Row with Progress Pie

    @ViewBuilder
    private func projectRow(_ project: Project) -> some View {
        SidebarProjectRow(
            project: project,
            isSelected: viewModel.selectedProject?.id == project.id,
            onSelect: {
                withAnimation(.easeInOut(duration: 0.15)) {
                    viewModel.selectProject(project)
                }
            }
        )
        .dropDestination(for: String.self) { items, _ in
            // Move dragged todos into this project
            for idString in items {
                if let uuid = UUID(uuidString: idString),
                   let todo = allTodos.first(where: { $0.id == uuid }) {
                    todo.project = project
                }
            }
            return true
        }
        .contextMenu {
            Button("Завершить проект") {
                project.status = .completed
            }
            Button("Потом") {
                project.status = .someday
            }
            Divider()
            Button("Удалить проект", role: .destructive) {
                modelContext.delete(project)
            }
        }
    }

    // MARK: - Area Row

    @ViewBuilder
    private func areaRow(_ area: Area) -> some View {
        let isCollapsed = viewModel.isAreaCollapsed(area)

        HStack(spacing: 10) {
            Image(systemName: isCollapsed ? "chevron.right" : "chevron.down")
                .font(.system(size: 9, weight: .semibold))
                .foregroundStyle(.tertiary)
                .frame(width: 12)

            Text(area.title)
                .font(.system(size: 12, weight: .semibold))
                .foregroundStyle(.secondary)
                .textCase(.uppercase)
                .tracking(0.5)

            Spacer()
        }
        .padding(.vertical, 2)
        .contentShape(Rectangle())
        .listRowBackground(Color.clear)
        .onTapGesture {
            if NSEvent.modifierFlags.contains(.command) {
                if NSEvent.modifierFlags.contains(.option) {
                    viewModel.soloArea(area, allAreas: areas)
                } else {
                    if isCollapsed {
                        viewModel.expandAllAreas()
                    } else {
                        viewModel.collapseAllAreas(areas)
                    }
                }
            } else {
                withAnimation(.easeInOut(duration: 0.2)) {
                    viewModel.toggleAreaCollapse(area)
                }
            }
        }
        .dropDestination(for: String.self) { items, _ in
            for idString in items {
                if let uuid = UUID(uuidString: idString),
                   let todo = allTodos.first(where: { $0.id == uuid }) {
                    todo.area = area
                }
            }
            return true
        }
    }

    // MARK: - Bottom Bar

    private var bottomBar: some View {
        HStack {
            Button {
                viewModel.isShowingNewProject = true
            } label: {
                HStack(spacing: 4) {
                    Image(systemName: "plus")
                        .font(.system(size: 11, weight: .semibold))
                    Text("Новый список")
                        .font(.system(size: 12))
                }
            }
            .buttonStyle(.plain)
            .foregroundStyle(.secondary)
            .padding(.horizontal, 20)
            .padding(.vertical, 10)

            Spacer()
        }
    }

    // MARK: - Helpers

    private func projectColor(_ project: Project) -> Color {
        if let colorTag = project.colorTag {
            switch colorTag {
            case "red": return .red
            case "orange": return .orange
            case "green": return .green
            case "purple": return .purple
            case "pink": return .pink
            default: return .blue
            }
        }
        return .blue
    }

    private func countFor(_ list: SmartList) -> Int {
        TodoFilterService.count(for: list, in: allTodos)
    }
}

// MARK: - Sidebar Row with Hover

private struct SidebarSmartListRow: View {
    let list: SmartList
    let isSelected: Bool
    let count: Int
    let onSelect: () -> Void

    @State private var isHovered = false

    var body: some View {
        HStack(spacing: 8) {
            Image(systemName: list.systemImage)
                .font(.system(size: 14, weight: .medium))
                .foregroundStyle(list.iconColor)
                .frame(width: 20)

            Text(list.title)
                .font(.system(size: 13, weight: isSelected ? .semibold : .regular))

            Spacer()

            if count > 0 {
                Text("\(count)")
                    .font(.system(size: 12, design: .rounded).weight(.medium))
                    .foregroundStyle(.secondary)
                    .monospacedDigit()
            }
        }
        .padding(.vertical, 2)
        .padding(.horizontal, 4)
        .contentShape(Rectangle())
        .listRowBackground(
            RoundedRectangle(cornerRadius: 6)
                .fill(isSelected ? Color.accentColor.opacity(0.15) : (isHovered ? Color.primary.opacity(0.06) : .clear))
                .padding(.horizontal, 2)
        )
        .onTapGesture { onSelect() }
        .onHover { isHovered = $0 }
    }
}

private struct SidebarProjectRow: View {
    let project: Project
    let isSelected: Bool
    let onSelect: () -> Void

    @State private var isHovered = false

    var body: some View {
        HStack(spacing: 8) {
            ProgressView(value: project.progress)
                .progressViewStyle(.circular)
                .controlSize(.mini)
                .frame(width: 20)

            Text(project.title)
                .font(.system(size: 13, weight: isSelected ? .semibold : .regular))
                .lineLimit(1)

            Spacer()

            let activeCount = project.todoItems.filter { !$0.isCompleted && !$0.isCancelled && !$0.isTrashed }.count
            if activeCount > 0 {
                Text("\(activeCount)")
                    .font(.system(size: 12, design: .rounded).weight(.medium))
                    .foregroundStyle(.secondary)
                    .monospacedDigit()
            }
        }
        .padding(.vertical, 2)
        .padding(.horizontal, 4)
        .contentShape(Rectangle())
        .listRowBackground(
            RoundedRectangle(cornerRadius: 6)
                .fill(isSelected ? Color.accentColor.opacity(0.15) : (isHovered ? Color.primary.opacity(0.06) : .clear))
                .padding(.horizontal, 2)
        )
        .onTapGesture { onSelect() }
        .onHover { isHovered = $0 }
    }
}

// MARK: - Progress Pie (Things 3 style)

struct ProjectPieProgress: View {
    let progress: Double
    var size: CGFloat = 14
    var color: Color = .blue

    var body: some View {
        ZStack {
            Circle()
                .strokeBorder(color.opacity(0.25), lineWidth: 1.5)
                .frame(width: size, height: size)

            if progress > 0 {
                PieShape(progress: min(progress, 1.0))
                    .fill(color.opacity(0.6))
                    .frame(width: size - 3, height: size - 3)
            }
        }
        .frame(width: size, height: size)
    }
}

struct PieShape: Shape {
    var progress: Double

    var animatableData: Double {
        get { progress }
        set { progress = newValue }
    }

    func path(in rect: CGRect) -> Path {
        var path = Path()
        let center = CGPoint(x: rect.midX, y: rect.midY)
        let radius = min(rect.width, rect.height) / 2
        let startAngle = Angle(degrees: -90)
        let endAngle = Angle(degrees: -90 + 360 * progress)

        path.move(to: center)
        path.addArc(center: center, radius: radius, startAngle: startAngle, endAngle: endAngle, clockwise: false)
        path.closeSubpath()
        return path
    }
}
