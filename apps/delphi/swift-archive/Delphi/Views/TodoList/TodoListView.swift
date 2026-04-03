import SwiftUI
import SwiftData

struct TodoListView: View {
    @Bindable var viewModel: TodoListViewModel
    @Environment(\.modelContext) private var modelContext
    @Environment(ArkSyncClient.self) private var syncClient
    @Query private var allTodos: [TodoItem]
    @Query(sort: \Tag.title) private var allTags: [Tag]
    @FocusState private var isNewTodoFieldFocused: Bool

    let smartList: SmartList?
    let project: Project?

    // MARK: - Cached filtered data

    @State private var cachedTodos: [TodoItem] = []
    @State private var cachedTodayMain: [TodoItem] = []
    @State private var cachedTodayEvening: [TodoItem] = []

    @State private var expandedTodoID: UUID?

    var canCreateTodo: Bool {
        smartList != .trash && smartList != .logbook
    }

    var showsHeadings: Bool { project != nil }
    var showsEvening: Bool { smartList == .today }

    // MARK: - Body

    var body: some View {
        ZStack(alignment: .top) {
            VStack(spacing: 0) {
                listHeader
                    .padding(.horizontal, 28)
                    .padding(.top, 24)
                    .padding(.bottom, 12)

                // Multi-select action bar
                if viewModel.hasMultiSelection {
                    multiSelectBar
                        .padding(.horizontal, 20)
                        .padding(.bottom, 8)
                }

                if !allTags.isEmpty && smartList != .logbook && smartList != .trash && !viewModel.hasMultiSelection {
                    tagFilterBar
                        .padding(.horizontal, 20)
                        .padding(.bottom, 8)
                }

                if cachedTodos.isEmpty && !viewModel.isCreatingNewTodo {
                    if smartList != nil {
                        emptyState
                            .frame(maxHeight: .infinity)
                    }
                    Spacer()
                        .contentShape(Rectangle())
                        .onTapGesture { dismissCreation() }
                } else {
                    ScrollView {
                        LazyVStack(spacing: 1) {
                            if viewModel.isCreatingNewTodo && canCreateTodo {
                                newTodoField
                            }

                            if showsHeadings {
                                projectContent
                            } else if showsEvening {
                                todayContent
                            } else {
                                plainContent
                            }

                            Color.clear
                                .frame(maxWidth: .infinity, minHeight: 200)
                                .contentShape(Rectangle())
                                .onTapGesture {
                                    dismissCreation()
                                    viewModel.clearSelection()
                                }
                        }
                        .padding(.horizontal, 16)
                        .padding(.top, 4)
                    }
                }
            }

        }
        .frame(maxWidth: .infinity, maxHeight: .infinity)
        .safeAreaInset(edge: .bottom, spacing: 0) {
            bottomToolbar
        }
        .toolbar(.hidden, for: .automatic)
        .toolbarBackground(.hidden)
        // Recompute cache when inputs change
        .onChange(of: allTodos.map(\.isCompleted)) { _, _ in refilter() }
        .onChange(of: allTodos.map(\.isTrashed)) { _, _ in refilter() }
        .onChange(of: allTodos.map(\.isToday)) { _, _ in refilter() }
        .onChange(of: allTodos.map(\.isSomeday)) { _, _ in refilter() }
        .onChange(of: allTodos.count) { _, _ in refilter() }
        .onChange(of: smartList) { _, _ in dismissCreation(); viewModel.clearSelection(); refilter() }
        .onChange(of: project?.id) { _, _ in dismissCreation(); viewModel.clearSelection(); refilter() }
        .onChange(of: viewModel.activeTagFilters) { _, _ in refilter() }
        .onChange(of: viewModel.sortMode) { _, _ in refilter() }
        .onAppear { refilter() }
        // Keyboard shortcuts
        .onKeyPress(.escape) { handleEscape() }
        .onKeyPress(.return) { handleReturn() }
        .onKeyPress(.upArrow) { moveSelection(up: true); return .handled }
        .onKeyPress(.downArrow) { moveSelection(up: false); return .handled }
        .onKeyPress(.delete) { handleDelete() }
        .modifier(TodoCommandHandlers(viewModel: viewModel, cachedTodos: cachedTodos, modelContext: modelContext, syncClient: syncClient))
    }

    // MARK: - Cache

    private func refilter() {
        var result: [TodoItem]

        if let project {
            result = project.todoItems
                .filter { !$0.isTrashed }
                .sorted { $0.sortOrder < $1.sortOrder }
        } else if let smartList {
            result = TodoFilterService.filter(for: smartList, in: allTodos)
        } else {
            result = []
        }

        if !viewModel.activeTagFilters.isEmpty {
            result = result.filter { viewModel.matchesTagFilter($0) }
        }

        // Apply sort mode
        switch viewModel.sortMode {
        case .manual:
            break // keep existing order
        case .byDate:
            result.sort { ($0.scheduledDate ?? .distantFuture) < ($1.scheduledDate ?? .distantFuture) }
        case .byTitle:
            result.sort { $0.title.localizedCompare($1.title) == .orderedAscending }
        case .byCreated:
            result.sort { $0.createdAt > $1.createdAt }
        }

        cachedTodos = result

        if showsEvening {
            cachedTodayMain = result.filter { !$0.isEvening }
            cachedTodayEvening = result.filter(\.isEvening)
        }
    }

    // MARK: - Content Sections

    @ViewBuilder
    private var plainContent: some View {
        ForEach(cachedTodos) { todo in
            todoRow(todo)
        }
    }

    @ViewBuilder
    private var todayContent: some View {
        ForEach(cachedTodayMain) { todo in
            todoRow(todo)
        }

        if !cachedTodayEvening.isEmpty {
            eveningDivider
            ForEach(cachedTodayEvening) { todo in
                todoRow(todo)
            }
        }
    }

    @ViewBuilder
    private var projectContent: some View {
        let sections = headingSections
        ForEach(Array(sections.enumerated()), id: \.offset) { _, section in
            if let heading = section.heading {
                headingRow(heading)
            }
            ForEach(section.todos) { todo in
                todoRow(todo)
            }
        }
    }

    private var headingSections: [(heading: Heading?, todos: [TodoItem])] {
        guard let project else { return [] }
        let headings = project.headings.sorted { $0.sortOrder < $1.sortOrder }
        var sections: [(heading: Heading?, todos: [TodoItem])] = []

        let noHeading = cachedTodos.filter { $0.headingID == nil }
        if !noHeading.isEmpty {
            sections.append((heading: nil, todos: noHeading))
        }
        for heading in headings {
            let headingTodos = cachedTodos.filter { $0.headingID == heading.id }
            sections.append((heading: heading, todos: headingTodos))
        }
        return sections
    }

    // MARK: - Todo Row

    @ViewBuilder
    private func todoRow(_ todo: TodoItem) -> some View {
        let isCompleting = viewModel.completingTodoIDs.contains(todo.id)
        let isSelected = viewModel.selectedTodoIDs.contains(todo.id)

        TodoRowView(
            todo: todo,
            isSelected: isSelected,
            isExpanded: expandedTodoID == todo.id,
            onToggle: {
                withAnimation(.spring(response: 0.3, dampingFraction: 0.7)) {
                    viewModel.toggleCompletion(todo)
                }
                syncClient.sendTodoChange(todo, changeType: "update")
                if !todo.isCompleted { return }
                Task {
                    try? await Task.sleep(for: .milliseconds(800))
                    withAnimation(.easeOut(duration: 0.3)) {
                        viewModel.removeCompletedFromView(todo.id)
                    }
                }
            },
            onSelect: {
                if NSEvent.modifierFlags.contains(.command) {
                    viewModel.selectTodo(todo, extend: true)
                } else if NSEvent.modifierFlags.contains(.shift) {
                    viewModel.extendSelection(to: todo, in: cachedTodos)
                } else {
                    viewModel.selectTodo(todo)
                }
                viewModel.editingTodoID = nil
            },
            onToggleExpand: {
                withAnimation(.spring(response: 0.25, dampingFraction: 0.8)) {
                    expandedTodoID = expandedTodoID == todo.id ? nil : todo.id
                }
            },
            isEditing: viewModel.editingTodoID == todo.id,
            onStopEditing: {
                viewModel.editingTodoID = nil
            }
        )
        .opacity(expandedTodoID != nil && expandedTodoID != todo.id ? 0.4 : 1.0)
        .opacity(isCompleting && todo.isCompleted ? 0.5 : 1.0)
        .transition(.asymmetric(
            insertion: .opacity,
            removal: .opacity.combined(with: .move(edge: .top))
        ))
        .contextMenu {
            todoContextMenu(for: todo)
        }
        .draggable(todo.id.uuidString) {
            Text(todo.title)
                .padding(8)
                .background(.ultraThinMaterial)
                .clipShape(RoundedRectangle(cornerRadius: 8))
        }
    }

    // MARK: - Heading Row

    @ViewBuilder
    private func headingRow(_ heading: Heading) -> some View {
        HStack(spacing: 0) {
            Text(heading.title)
                .font(.system(size: 13, weight: .bold))
                .foregroundStyle(.secondary)
                .textCase(.uppercase)
                .tracking(0.5)
            Spacer()
        }
        .padding(.horizontal, 14)
        .padding(.top, 20)
        .padding(.bottom, 6)
        .contextMenu {
            Button("Переименовать…") { }
            Divider()
            Button("Удалить заголовок", role: .destructive) {
                modelContext.delete(heading)
            }
        }
    }

    // MARK: - Multi-Select Action Bar

    private var multiSelectBar: some View {
        HStack(spacing: 12) {
            Text("\(viewModel.selectedTodoIDs.count) выбрано")
                .font(.system(size: 12, weight: .medium))
                .foregroundStyle(.secondary)

            Spacer()

            Button {
                withAnimation(.spring(response: 0.3, dampingFraction: 0.7)) {
                    for todo in cachedTodos where viewModel.selectedTodoIDs.contains(todo.id) {
                        viewModel.toggleCompletion(todo)
                        syncClient.sendTodoChange(todo, changeType: "update")
                    }
                }
                viewModel.clearSelection()
            } label: {
                Label("Выполнить", systemImage: "checkmark")
                    .font(.system(size: 12))
            }
            .buttonStyle(.plain)

            Button {
                withAnimation {
                    for todo in cachedTodos where viewModel.selectedTodoIDs.contains(todo.id) {
                        viewModel.moveToTrash(todo)
                        syncClient.sendTodoChange(todo, changeType: "update")
                    }
                }
                viewModel.clearSelection()
            } label: {
                Label("Удалить", systemImage: "trash")
                    .font(.system(size: 12))
                    .foregroundStyle(.red)
            }
            .buttonStyle(.plain)

            Button {
                viewModel.clearSelection()
            } label: {
                Image(systemName: "xmark")
                    .font(.system(size: 11, weight: .medium))
                    .foregroundStyle(.secondary)
            }
            .buttonStyle(.plain)
        }
        .padding(.horizontal, 12)
        .padding(.vertical, 8)
        .background(.regularMaterial, in: RoundedRectangle(cornerRadius: 8))
    }

    // MARK: - Evening Divider

    private var eveningDivider: some View {
        HStack(spacing: 8) {
            Image(systemName: "moon.fill")
                .font(.system(size: 10))
                .foregroundStyle(.secondary.opacity(0.5))
            Text("Этим вечером")
                .font(.system(size: 12, weight: .medium))
                .foregroundStyle(.secondary.opacity(0.6))
            VStack { Divider() }
        }
        .padding(.horizontal, 14)
        .padding(.top, 16)
        .padding(.bottom, 4)
    }

    // MARK: - Tag Filter Bar

    @ViewBuilder
    private var tagFilterBar: some View {
        let usedTags = allTags.filter { tag in
            cachedTodos.contains { $0.tags.contains { $0.id == tag.id } }
        }
        if !usedTags.isEmpty {
            ScrollView(.horizontal, showsIndicators: false) {
                HStack(spacing: 6) {
                    if !viewModel.activeTagFilters.isEmpty {
                        Button {
                            viewModel.clearTagFilters()
                        } label: {
                            Text("Все")
                                .font(.system(size: 11, weight: .medium))
                                .padding(.horizontal, 8)
                                .padding(.vertical, 4)
                                .background(Capsule().fill(Color.primary.opacity(0.08)))
                                .foregroundStyle(.secondary)
                        }
                        .buttonStyle(.plain)
                    }

                    ForEach(usedTags) { tag in
                        let isActive = viewModel.activeTagFilters.contains(tag.id)
                        Button {
                            if NSEvent.modifierFlags.contains(.command) {
                                viewModel.isTagFilterAND = true
                            }
                            viewModel.toggleTagFilter(tag)
                        } label: {
                            Text(tag.title)
                                .font(.system(size: 11, weight: isActive ? .semibold : .regular))
                                .padding(.horizontal, 8)
                                .padding(.vertical, 4)
                                .background(
                                    Capsule().fill(isActive ? Color.accentColor.opacity(0.15) : Color.primary.opacity(0.05))
                                )
                                .foregroundStyle(isActive ? Color.accentColor : .secondary)
                        }
                        .buttonStyle(.plain)
                    }
                }
            }
        }
    }

    // MARK: - Header

    @ViewBuilder
    private var listHeader: some View {
        HStack(spacing: 10) {
            if let smartList {
                Image(systemName: smartList.systemImage)
                    .font(.system(size: 24, weight: .medium))
                    .foregroundStyle(smartList.iconColor)

                Text(smartList.title)
                    .font(.system(size: 26, weight: .bold))
                    .foregroundStyle(.primary)
            } else if let project {
                ProgressView(value: project.progress)
                    .progressViewStyle(.circular)
                    .controlSize(.small)

                Text(project.title)
                    .font(.system(size: 26, weight: .bold))
                    .foregroundStyle(.primary)
            }

            Spacer()

            // Sort menu
            Menu {
                ForEach(TodoListViewModel.SortMode.allCases, id: \.self) { mode in
                    Button {
                        viewModel.sortMode = mode
                    } label: {
                        if viewModel.sortMode == mode {
                            Label(mode.rawValue, systemImage: "checkmark")
                        } else {
                            Text(mode.rawValue)
                        }
                    }
                }
            } label: {
                Image(systemName: "arrow.up.arrow.down")
                    .font(.system(size: 13))
                    .foregroundStyle(.secondary)
            }
            .buttonStyle(.plain)
            .help("Сортировка")
        }
    }

    // MARK: - New Todo Field

    @ViewBuilder
    private var newTodoField: some View {
        HStack(spacing: 12) {
            Toggle("", isOn: .constant(false))
                .toggleStyle(.checkbox)
                .labelsHidden()
                .opacity(0.3)

            TextField("Новая задача", text: $viewModel.newTodoTitle)
                .textFieldStyle(.plain)
                .font(.system(size: 14))
                .focused($isNewTodoFieldFocused)
                .onSubmit { createTodo() }
                .onExitCommand { viewModel.isCreatingNewTodo = false }
        }
        .padding(.horizontal, 14)
        .padding(.vertical, 10)
        .background(
            RoundedRectangle(cornerRadius: 10)
                .fill(Color.accentColor.opacity(0.07))
        )
        .padding(.vertical, 2)
    }

    // MARK: - Empty State

    @ViewBuilder
    private var emptyState: some View {
        if let smartList {
            ContentUnavailableView(
                emptyTitle(for: smartList),
                systemImage: smartList.systemImage,
                description: Text(emptySubtitle(for: smartList))
            )
        }
    }

    private func emptyTitle(for list: SmartList) -> String {
        switch list {
        case .inbox: "Входящие пусты"
        case .today: "День свободен"
        case .upcoming: "Ничего не запланировано"
        case .anytime: "Нет активных задач"
        case .someday: "Пока ничего нет"
        case .logbook: "Нет завершённых задач"
        case .trash: "Корзина пуста"
        }
    }

    private func emptySubtitle(for list: SmartList) -> String {
        switch list {
        case .inbox: "Нажмите + чтобы добавить задачу"
        case .today: "Задачи на сегодня появятся здесь"
        case .upcoming: "Запланируйте задачу с датой"
        case .anytime: "Все задачи выполнены"
        case .someday: "Перенесите задачу в «Потом»"
        case .logbook: "Завершённые задачи появятся здесь"
        case .trash: "Удалённые задачи появятся здесь"
        }
    }

    // MARK: - Context Menu (with keyboard shortcut hints)

    @ViewBuilder
    private func todoContextMenu(for todo: TodoItem) -> some View {
        Button(todo.isToday ? "Убрать из Сегодня" : "На сегодня  ⌘T") {
            viewModel.toggleToday(todo)
            syncClient.sendTodoChange(todo, changeType: "update")
        }
        Button(todo.isEvening ? "Убрать из Вечера" : "Этим вечером  ⌘E") {
            viewModel.toggleEvening(todo)
            syncClient.sendTodoChange(todo, changeType: "update")
        }
        Divider()
        Button("Когда-нибудь  ⌘O") {
            viewModel.setSomeday(todo)
            syncClient.sendTodoChange(todo, changeType: "update")
        }
        Divider()
        Menu("Приоритет") {
            ForEach(Priority.allCases) { priority in
                Button {
                    todo.priority = priority
                    syncClient.sendTodoChange(todo, changeType: "update")
                } label: {
                    if todo.priority == priority {
                        Label(priority.label, systemImage: "checkmark")
                    } else {
                        Text(priority.label)
                    }
                }
            }
        }
        Divider()
        Button("Выполнить  ⌘K") {
            withAnimation(.spring(response: 0.3, dampingFraction: 0.7)) {
                viewModel.toggleCompletion(todo)
            }
            syncClient.sendTodoChange(todo, changeType: "update")
        }
        Button("Дублировать  ⌘D") {
            let copy = todo.duplicate()
            modelContext.insert(copy)
            syncClient.sendTodoChange(copy, changeType: "create")
        }
        Button("Переместить…  ⇧⌘M") {
            viewModel.selectTodo(todo)
            NotificationCenter.default.post(name: .moveTodo, object: nil)
        }
        Divider()
        if todo.isTrashed {
            Button("Восстановить") {
                todo.restore()
                syncClient.sendTodoChange(todo, changeType: "update")
            }
            Button("Удалить навсегда  ⌘⌫", role: .destructive) {
                syncClient.sendTodoChange(todo, changeType: "delete")
                modelContext.delete(todo)
            }
        } else {
            Button("В корзину  ⌘⌫", role: .destructive) {
                viewModel.moveToTrash(todo)
                syncClient.sendTodoChange(todo, changeType: "update")
            }
        }
    }

    // MARK: - Bottom Toolbar

    private var bottomToolbar: some View {
        HStack(spacing: 0) {
            if canCreateTodo {
                Button {
                    viewModel.startCreatingTodo()
                    isNewTodoFieldFocused = true
                } label: {
                    Image(systemName: "plus")
                        .font(.system(size: 15, weight: .medium))
                        .foregroundStyle(.secondary)
                        .frame(width: 44, height: 32)
                        .contentShape(Rectangle())
                }
                .buttonStyle(.plain)
                .help("Новая задача (⌘N)")
            }

            Spacer()

            Button {
                NotificationCenter.default.post(name: .quickFind, object: nil)
            } label: {
                Image(systemName: "magnifyingglass")
                    .font(.system(size: 13, weight: .medium))
                    .foregroundStyle(.secondary)
                    .frame(width: 44, height: 32)
                    .contentShape(Rectangle())
            }
            .buttonStyle(.plain)
            .help("Быстрый поиск (⌘F)")
        }
        .padding(.horizontal, 16)
        .padding(.vertical, 4)
    }

    // MARK: - Keyboard Handlers

    private func handleEscape() -> KeyPress.Result {
        if viewModel.editingTodoID != nil {
            viewModel.editingTodoID = nil
            return .handled
        }
        if expandedTodoID != nil {
            withAnimation(.spring(response: 0.25, dampingFraction: 0.8)) {
                expandedTodoID = nil
            }
            return .handled
        }
        if viewModel.isCreatingNewTodo {
            dismissCreation()
            return .handled
        }
        if !viewModel.selectedTodoIDs.isEmpty {
            viewModel.clearSelection()
            return .handled
        }
        if !viewModel.activeTagFilters.isEmpty {
            viewModel.clearTagFilters()
            return .handled
        }
        return .ignored
    }

    private func handleReturn() -> KeyPress.Result {
        // Return while editing → stop editing
        if viewModel.editingTodoID != nil {
            viewModel.editingTodoID = nil
            return .handled
        }
        // Return on selected task → start inline editing
        if let todo = singleSelectedTodo {
            viewModel.editingTodoID = todo.id
            return .handled
        }
        return .ignored
    }

    private func handleDelete() -> KeyPress.Result {
        guard let todo = singleSelectedTodo else { return .ignored }
        if NSEvent.modifierFlags.contains(.command) {
            withAnimation { viewModel.moveToTrash(todo) }
            return .handled
        }
        return .ignored
    }

    // MARK: - Helpers

    private func applyToFirstSelected(_ action: (TodoItem) -> Void) {
        guard let todo = singleSelectedTodo else { return }
        action(todo)
    }

    private var singleSelectedTodo: TodoItem? {
        guard viewModel.selectedTodoIDs.count == 1,
              let id = viewModel.selectedTodoIDs.first
        else { return nil }
        return cachedTodos.first { $0.id == id }
    }

    private func moveSelection(up: Bool) {
        let todos = cachedTodos
        guard !todos.isEmpty else { return }

        if viewModel.selectedTodoIDs.isEmpty {
            viewModel.selectTodo(up ? todos.last! : todos.first!)
            return
        }

        guard let currentID = viewModel.selectedTodoIDs.first,
              let currentIdx = todos.firstIndex(where: { $0.id == currentID })
        else { return }

        let newIdx = up ? max(0, currentIdx - 1) : min(todos.count - 1, currentIdx + 1)
        viewModel.selectTodo(todos[newIdx])
    }

    private func dismissCreation() {
        viewModel.isCreatingNewTodo = false
        viewModel.newTodoTitle = ""
    }

    private func createTodo() {
        let title = viewModel.newTodoTitle.trimmingCharacters(in: .whitespaces)
        guard !title.isEmpty else {
            viewModel.isCreatingNewTodo = false
            return
        }
        let todo = TodoItem(title: title, project: project)
        if smartList == .today { todo.isToday = true }
        else if smartList == .someday { todo.isSomeday = true }
        modelContext.insert(todo)
        syncClient.sendTodoChange(todo, changeType: "create")
        viewModel.newTodoTitle = ""
        viewModel.isCreatingNewTodo = false
    }
}

// MARK: - Command Notification Handlers (extracted to avoid type-check timeout)

private struct TodoCommandHandlers: ViewModifier {
    let viewModel: TodoListViewModel
    let cachedTodos: [TodoItem]
    let modelContext: ModelContext
    let syncClient: ArkSyncClient

    func body(content: Content) -> some View {
        content
            .onReceive(NotificationCenter.default.publisher(for: .setToday)) { _ in
                apply { viewModel.toggleToday($0); syncClient.sendTodoChange($0, changeType: "update") }
            }
            .onReceive(NotificationCenter.default.publisher(for: .setEvening)) { _ in
                apply { viewModel.toggleEvening($0); syncClient.sendTodoChange($0, changeType: "update") }
            }
            .onReceive(NotificationCenter.default.publisher(for: .setSomeday)) { _ in
                apply { viewModel.setSomeday($0); syncClient.sendTodoChange($0, changeType: "update") }
            }
            .onReceive(NotificationCenter.default.publisher(for: .setAnytime)) { _ in
                apply { viewModel.setAnytime($0); syncClient.sendTodoChange($0, changeType: "update") }
            }
            .onReceive(NotificationCenter.default.publisher(for: .completeTodo)) { _ in
                apply { todo in
                    withAnimation(.spring(response: 0.3, dampingFraction: 0.7)) {
                        viewModel.toggleCompletion(todo)
                    }
                    syncClient.sendTodoChange(todo, changeType: "update")
                }
            }
            .onReceive(NotificationCenter.default.publisher(for: .cancelTodo)) { _ in
                apply { todo in
                    withAnimation(.spring(response: 0.3, dampingFraction: 0.7)) {
                        viewModel.cancelTodo(todo)
                    }
                    syncClient.sendTodoChange(todo, changeType: "update")
                }
            }
            .onReceive(NotificationCenter.default.publisher(for: .duplicateTodo)) { _ in
                apply { todo in
                    let copy = todo.duplicate()
                    modelContext.insert(copy)
                    syncClient.sendTodoChange(copy, changeType: "create")
                }
            }
    }

    private func apply(_ action: (TodoItem) -> Void) {
        guard viewModel.selectedTodoIDs.count == 1,
              let id = viewModel.selectedTodoIDs.first,
              let todo = cachedTodos.first(where: { $0.id == id })
        else { return }
        action(todo)
    }
}
