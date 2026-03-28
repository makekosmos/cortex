import SwiftUI
import SwiftData
import AppKit

// MARK: - Quick Entry SwiftUI View

struct QuickEntryView: View {
    @Environment(\.modelContext) private var modelContext
    @Environment(ArkSyncClient.self) private var syncClient
    @Query(sort: \Project.sortOrder) private var projects: [Project]
    @Query(sort: \Tag.title) private var allTags: [Tag]

    @State private var title = ""
    @State private var notes = ""
    @State private var whenText = ""
    @State private var scheduledDate: Date?
    @State private var isToday = false
    @State private var isEvening = false
    @State private var selectedProject: Project?
    @State private var selectedTags: [Tag] = []
    @State private var priority: Priority = .none
    @FocusState private var isTitleFocused: Bool

    let onSave: () -> Void
    let onDismiss: () -> Void

    var body: some View {
        VStack(alignment: .leading, spacing: 12) {
            // Title
            TextField("Новая задача", text: $title)
                .textFieldStyle(.plain)
                .font(.system(size: 18, weight: .semibold))
                .focused($isTitleFocused)
                .onSubmit { saveTodo() }

            // Notes
            TextField("Заметки", text: $notes, axis: .vertical)
                .textFieldStyle(.plain)
                .font(.system(size: 14))
                .foregroundStyle(.secondary)
                .lineLimit(3)

            Divider().opacity(0.5)

            // Metadata row
            HStack(spacing: 8) {
                // When
                HStack(spacing: 4) {
                    Image(systemName: "calendar")
                        .font(.system(size: 10))
                    TextField("Когда", text: $whenText)
                        .textFieldStyle(.plain)
                        .font(.system(size: 11))
                        .frame(width: 80)
                        .onSubmit {
                            if let date = NaturalDateParser.parse(whenText) {
                                scheduledDate = date
                            }
                        }
                }
                .padding(.horizontal, 8)
                .padding(.vertical, 5)
                .background(Capsule().fill(Color.primary.opacity(0.05)))
                .foregroundStyle(scheduledDate != nil ? Color.accentColor : Color.secondary)

                // Today
                Button {
                    isToday.toggle()
                    if isToday { isEvening = false }
                } label: {
                    HStack(spacing: 4) {
                        Image(systemName: "star.fill")
                            .font(.system(size: 10))
                        Text("Сегодня")
                            .font(.system(size: 11))
                    }
                    .padding(.horizontal, 8)
                    .padding(.vertical, 5)
                    .background(Capsule().fill(isToday ? .yellow.opacity(0.15) : Color.primary.opacity(0.05)))
                    .foregroundStyle(isToday ? .yellow : Color.secondary)
                }
                .buttonStyle(.plain)

                // Evening
                Button {
                    isEvening.toggle()
                    if isEvening { isToday = true }
                } label: {
                    HStack(spacing: 4) {
                        Image(systemName: "moon.fill")
                            .font(.system(size: 10))
                        Text("Вечер")
                            .font(.system(size: 11))
                    }
                    .padding(.horizontal, 8)
                    .padding(.vertical, 5)
                    .background(Capsule().fill(isEvening ? Color.indigo.opacity(0.12) : Color.primary.opacity(0.05)))
                    .foregroundStyle(isEvening ? .indigo : Color.secondary)
                }
                .buttonStyle(.plain)

                Spacer()

                // Project
                Menu {
                    Button("Входящие") { selectedProject = nil }
                    Divider()
                    ForEach(projects) { project in
                        Button(project.title) { selectedProject = project }
                    }
                } label: {
                    HStack(spacing: 4) {
                        Image(systemName: "folder")
                            .font(.system(size: 10))
                        Text(selectedProject?.title ?? "Входящие")
                            .font(.system(size: 11))
                    }
                    .padding(.horizontal, 8)
                    .padding(.vertical, 5)
                    .background(Capsule().fill(Color.primary.opacity(0.05)))
                    .foregroundStyle(selectedProject == nil ? Color.secondary : Color.accentColor)
                }
                .buttonStyle(.plain)
            }
        }
        .padding(20)
        .frame(width: 480)
        .background(
            RoundedRectangle(cornerRadius: 14)
                .fill(.regularMaterial)
                .shadow(color: .black.opacity(0.2), radius: 20, y: 8)
        )
        .onAppear { isTitleFocused = true }
        .onExitCommand { onDismiss() }
    }

    private func saveTodo() {
        let trimmed = title.trimmingCharacters(in: .whitespaces)
        guard !trimmed.isEmpty else {
            onDismiss()
            return
        }

        let todo = TodoItem(
            title: trimmed,
            notes: notes.isEmpty ? nil : notes,
            priority: priority,
            scheduledDate: scheduledDate,
            isToday: isToday,
            isEvening: isEvening,
            project: selectedProject
        )
        for tag in selectedTags { todo.tags.append(tag) }
        modelContext.insert(todo)
        syncClient.sendTodoChange(todo, changeType: "create")
        onSave()
    }
}

// MARK: - NSPanel wrapper for floating Quick Entry

@MainActor
class QuickEntryPanelController {
    private var panel: NSPanel?
    private var modelContainer: ModelContainer?
    private var syncClient: ArkSyncClient?

    static let shared = QuickEntryPanelController()

    func setup(container: ModelContainer, syncClient: ArkSyncClient) {
        self.modelContainer = container
        self.syncClient = syncClient
    }

    func toggle() {
        if let panel, panel.isVisible {
            dismiss()
        } else {
            show()
        }
    }

    func show() {
        guard let container = modelContainer, let syncClient else { return }

        let view = QuickEntryView(
            onSave: { [weak self] in self?.dismiss() },
            onDismiss: { [weak self] in self?.dismiss() }
        )
        .modelContainer(container)
        .environment(syncClient)

        let hostingView = NSHostingView(rootView: view)
        hostingView.frame = NSRect(x: 0, y: 0, width: 480, height: 200)

        let panel = NSPanel(
            contentRect: NSRect(x: 0, y: 0, width: 480, height: 200),
            styleMask: [.nonactivatingPanel, .titled, .closable, .fullSizeContentView],
            backing: .buffered,
            defer: true
        )
        panel.titleVisibility = .hidden
        panel.titlebarAppearsTransparent = true
        panel.isMovableByWindowBackground = true
        panel.level = .floating
        panel.backgroundColor = .clear
        panel.isOpaque = false
        panel.hasShadow = true
        panel.contentView = hostingView
        panel.center()

        // Position near top of screen
        if let screen = NSScreen.main {
            let x = (screen.frame.width - 480) / 2
            let y = screen.frame.height * 0.7
            panel.setFrameOrigin(NSPoint(x: x, y: y))
        }

        panel.makeKeyAndOrderFront(nil)
        self.panel = panel
    }

    func dismiss() {
        panel?.close()
        panel = nil
    }
}
