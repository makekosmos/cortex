import SwiftUI
import SwiftData

struct TodoRowView: View {
    @Bindable var todo: TodoItem
    let isSelected: Bool
    let isExpanded: Bool
    let onToggle: () -> Void
    let onSelect: () -> Void
    let onToggleExpand: () -> Void
    var isEditing: Bool = false
    var onStopEditing: (() -> Void)?

    @Environment(\.modelContext) private var modelContext
    @Environment(ArkSyncClient.self) private var syncClient
    @Query(sort: \Project.sortOrder) private var projects: [Project]
    @Query(sort: \Tag.title) private var allTags: [Tag]

    @State private var isHovered = false
    @State private var newChecklistTitle = ""
    @FocusState private var isTitleFocused: Bool

    var body: some View {
        VStack(alignment: .leading, spacing: 0) {
            // Main row
            mainRow

            // Expanded details — inline below the row
            if isExpanded {
                expandedDetails
                    .padding(.leading, 40) // align with text (past checkbox)
                    .padding(.trailing, 14)
                    .padding(.bottom, 10)
                    .transition(.opacity.combined(with: .move(edge: .top)))
            }
        }
        .background(backgroundColor)
        .clipShape(RoundedRectangle(cornerRadius: 8))
        .contentShape(Rectangle())
        .onHover { isHovered = $0 }
        .onChange(of: isEditing) { _, editing in
            if editing {
                Task {
                    try? await Task.sleep(for: .milliseconds(50))
                    isTitleFocused = true
                }
            }
        }
        .accessibilityElement(children: .combine)
        .accessibilityLabel(todo.title)
    }

    // MARK: - Main Row

    private var mainRow: some View {
        HStack(spacing: 12) {
            Toggle("", isOn: Binding(
                get: { todo.isCompleted || todo.isCancelled },
                set: { _ in onToggle() }
            ))
            .toggleStyle(.checkbox)
            .labelsHidden()
            .accessibilityLabel("Отметить выполненной")

            VStack(alignment: .leading, spacing: 3) {
                if isEditing {
                    TextField("", text: $todo.title)
                        .textFieldStyle(.plain)
                        .font(.system(size: 14))
                        .focused($isTitleFocused)
                        .onSubmit { syncClient.sendTodoChange(todo, changeType: "update"); onStopEditing?() }
                        .onExitCommand { syncClient.sendTodoChange(todo, changeType: "update"); onStopEditing?() }
                } else {
                    Text(todo.title)
                        .font(.system(size: 14))
                        .strikethrough(todo.isCompleted || todo.isCancelled, color: .secondary.opacity(0.4))
                        .foregroundStyle(todo.isCompleted || todo.isCancelled ? .secondary : .primary)
                        .lineLimit(isExpanded ? nil : 2)
                }

                if !isExpanded, hasMetadata {
                    metadataRow
                }
            }

            Spacer(minLength: 4)

            if let project = todo.project, !isExpanded {
                Text(project.title)
                    .font(.system(size: 11))
                    .foregroundStyle(.tertiary)
                    .lineLimit(1)
            }

            // Expand chevron
            Image(systemName: isExpanded ? "chevron.down" : "chevron.right")
                .font(.system(size: 9, weight: .semibold))
                .foregroundStyle(.tertiary)
                .frame(width: 16)
                .highPriorityGesture(TapGesture().onEnded { withAnimation(.spring(response: 0.25, dampingFraction: 0.8)) { onToggleExpand() } })
        }
        .padding(.horizontal, 14)
        .padding(.vertical, 8)
        .onTapGesture(count: 2) {
            withAnimation(.spring(response: 0.25, dampingFraction: 0.8)) {
                onToggleExpand()
            }
        }
        .onTapGesture(count: 1) { onSelect() }
    }

    // MARK: - Expanded Details (inline)

    @ViewBuilder
    private var expandedDetails: some View {
        VStack(alignment: .leading, spacing: 12) {
            // Notes
            TextEditor(text: Binding(
                get: { todo.notes ?? "" },
                set: { todo.notes = $0.isEmpty ? nil : $0 }
            ))
            .font(.system(size: 13))
            .foregroundStyle(.primary)
            .scrollContentBackground(.hidden)
            .frame(minHeight: 40, maxHeight: 100)
            .padding(6)
            .background(RoundedRectangle(cornerRadius: 6).fill(Color.primary.opacity(0.03)))
            .overlay(alignment: .topLeading) {
                if todo.notes == nil || (todo.notes?.isEmpty ?? true) {
                    Text("Заметки…")
                        .font(.system(size: 13))
                        .foregroundStyle(.tertiary)
                        .padding(.horizontal, 10)
                        .padding(.vertical, 10)
                        .allowsHitTesting(false)
                }
            }

            // Checklist
            checklistSection

            Divider().opacity(0.3)

            // Metadata pills
            metadataPills
        }
        .padding(.top, 4)
        .onDisappear {
            syncClient.sendTodoChange(todo, changeType: "update")
        }
    }

    // MARK: - Checklist

    @ViewBuilder
    private var checklistSection: some View {
        VStack(alignment: .leading, spacing: 2) {
            ForEach(todo.checklistItems.sorted(by: { $0.sortOrder < $1.sortOrder })) { item in
                HStack(spacing: 8) {
                    Image(systemName: item.isCompleted ? "checkmark.circle.fill" : "circle")
                        .font(.system(size: 12))
                        .foregroundStyle(item.isCompleted ? Color.secondary : Color.secondary.opacity(0.4))
                        .onTapGesture { withAnimation { item.isCompleted.toggle() } }

                    TextField("", text: Binding(get: { item.title }, set: { item.title = $0 }))
                        .textFieldStyle(.plain)
                        .font(.system(size: 12))
                        .strikethrough(item.isCompleted)
                        .foregroundStyle(item.isCompleted ? .secondary : .primary)

                    Spacer()

                    Button { modelContext.delete(item) } label: {
                        Image(systemName: "xmark")
                            .font(.system(size: 8, weight: .semibold))
                            .foregroundStyle(.tertiary)
                    }
                    .buttonStyle(.plain)
                }
            }

            HStack(spacing: 8) {
                Image(systemName: "plus")
                    .font(.system(size: 10))
                    .foregroundStyle(.tertiary)
                TextField("Добавить пункт", text: $newChecklistTitle)
                    .textFieldStyle(.plain)
                    .font(.system(size: 12))
                    .foregroundStyle(.secondary)
                    .onSubmit {
                        let t = newChecklistTitle.trimmingCharacters(in: .whitespaces)
                        guard !t.isEmpty else { return }
                        let item = ChecklistItem(title: t, todoItem: todo)
                        item.sortOrder = todo.checklistItems.count
                        modelContext.insert(item)
                        newChecklistTitle = ""
                    }
            }
        }
    }

    // MARK: - Metadata Pills

    @ViewBuilder
    private var metadataPills: some View {
        HStack(spacing: 6) {
            // When
            MetadataPillCompact(icon: "calendar", label: todo.scheduledDate?.relativeDisplay ?? "Когда", isSet: todo.scheduledDate != nil)

            // Today
            Button {
                todo.isToday.toggle()
                if todo.isToday { todo.isSomeday = false }
                syncClient.sendTodoChange(todo, changeType: "update")
            } label: {
                Label("Сегодня", systemImage: "star.fill")
                    .font(.system(size: 11))
                    .foregroundStyle(todo.isToday ? .yellow : .secondary)
            }
            .buttonStyle(.plain)

            // Deadline
            MetadataPillCompact(icon: "flag.fill", label: todo.deadline?.relativeDisplay ?? "Дедлайн", isSet: todo.deadline != nil)

            Spacer()

            // Tags
            Menu {
                ForEach(allTags.filter { tag in !todo.tags.contains { $0.id == tag.id } }) { tag in
                    Button(tag.title) { todo.tags.append(tag); syncClient.sendTodoChange(todo, changeType: "update") }
                }
            } label: {
                Label(todo.tags.isEmpty ? "Теги" : todo.tags.map(\.title).joined(separator: ", "), systemImage: "tag")
                    .font(.system(size: 11))
                    .foregroundStyle(todo.tags.isEmpty ? .secondary : Color.accentColor)
            }
            .buttonStyle(.plain)

            // Project
            Menu {
                Button("Без проекта") { todo.project = nil; syncClient.sendTodoChange(todo, changeType: "update") }
                Divider()
                ForEach(projects) { p in
                    Button(p.title) { todo.project = p; syncClient.sendTodoChange(todo, changeType: "update") }
                }
            } label: {
                Label(todo.project?.title ?? "Проект", systemImage: "folder")
                    .font(.system(size: 11))
                    .foregroundStyle(todo.project == nil ? .secondary : Color.accentColor)
            }
            .buttonStyle(.plain)
        }
    }

    // MARK: - Collapsed Metadata Row

    @ViewBuilder
    private var metadataRow: some View {
        HStack(spacing: 8) {
            if todo.isToday && !todo.isEvening {
                Label("Сегодня", systemImage: "star.fill")
                    .font(.system(size: 11)).foregroundStyle(.yellow).labelStyle(.titleAndIcon)
            }
            if todo.isEvening {
                Label("Вечер", systemImage: "moon.fill")
                    .font(.system(size: 11)).foregroundStyle(.secondary.opacity(0.6)).labelStyle(.titleAndIcon)
            }
            if let date = todo.scheduledDate, !todo.isToday {
                Label(date.relativeDisplay, systemImage: "calendar")
                    .font(.system(size: 11)).foregroundStyle(date.isPast ? .red : .secondary).labelStyle(.titleAndIcon)
            }
            if let deadline = todo.deadline {
                Label(deadline.relativeDisplay, systemImage: "flag.fill")
                    .font(.system(size: 11)).foregroundStyle(deadline.isPast ? .red : .orange).labelStyle(.titleAndIcon)
            }
            if todo.notes != nil && !(todo.notes?.isEmpty ?? true) {
                Image(systemName: "note.text").font(.system(size: 9)).foregroundStyle(.tertiary)
            }
            if !todo.checklistItems.isEmpty {
                let done = todo.checklistItems.filter(\.isCompleted).count
                Label("\(done)/\(todo.checklistItems.count)", systemImage: "checklist")
                    .font(.system(size: 11)).foregroundStyle(.secondary).labelStyle(.titleAndIcon)
            }
            if todo.recurrenceRule != nil {
                Image(systemName: "repeat").font(.system(size: 9)).foregroundStyle(.tertiary)
            }
            ForEach(todo.tags) { tag in TagChip(tag: tag, compact: true) }
        }
    }

    private var hasMetadata: Bool {
        todo.isToday || todo.isEvening || todo.scheduledDate != nil ||
        todo.deadline != nil || (todo.notes != nil && !(todo.notes?.isEmpty ?? true)) ||
        !todo.checklistItems.isEmpty || !todo.tags.isEmpty || todo.recurrenceRule != nil
    }

    private var backgroundColor: Color {
        if isExpanded { return Color.accentColor.opacity(0.06) }
        if isSelected { return Color.accentColor.opacity(0.1) }
        if isHovered { return Color.primary.opacity(0.035) }
        return .clear
    }
}

// MARK: - Compact Metadata Pill

struct MetadataPillCompact: View {
    let icon: String
    let label: String
    let isSet: Bool

    var body: some View {
        Label(label, systemImage: icon)
            .font(.system(size: 11))
            .foregroundStyle(isSet ? Color.accentColor : .secondary)
    }
}
