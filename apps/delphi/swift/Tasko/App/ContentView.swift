import SwiftUI
import SwiftData

struct ContentView: View {
    @Environment(\.modelContext) private var modelContext
    @State private var sidebarVM = SidebarViewModel()
    @State private var todoListVM = TodoListViewModel()
    @State private var columnVisibility = NavigationSplitViewVisibility.doubleColumn
    @State private var isQuickOpenPresented = false
    @State private var isMoveDialogPresented = false

    var body: some View {
        ZStack(alignment: .top) {
            mainContent

            if isQuickOpenPresented {
                quickOpenOverlay
            }
        }
        .modifier(NotificationHandlers(
                sidebarVM: sidebarVM,
                todoListVM: todoListVM,
                modelContext: modelContext,
                isQuickOpenPresented: $isQuickOpenPresented,
                isMoveDialogPresented: $isMoveDialogPresented,
                columnVisibility: $columnVisibility
            ))
    }

    // MARK: - Main Layout

    private var mainContent: some View {
        NavigationSplitView(columnVisibility: $columnVisibility) {
            SidebarView(viewModel: sidebarVM)
                .navigationSplitViewColumnWidth(min: 200, ideal: 230, max: 280)
        } detail: {
            detailContent
        }
        .navigationSplitViewStyle(.prominentDetail)
        .sheet(isPresented: $isMoveDialogPresented) {
            MoveToDialog(
                todoIDs: todoListVM.selectedTodoIDs,
                onDismiss: { isMoveDialogPresented = false }
            )
        }
    }

    @ViewBuilder
    private var detailContent: some View {
        if sidebarVM.selectedSmartList == .upcoming {
            UpcomingView(viewModel: todoListVM)
        } else {
            TodoListView(
                viewModel: todoListVM,
                smartList: sidebarVM.selectedSmartList,
                project: sidebarVM.selectedProject
            )
        }
    }

    // MARK: - Quick Open (Spotlight-style overlay)

    private var quickOpenOverlay: some View {
        ZStack {
            Color.black.opacity(0.25)
                .ignoresSafeArea()
                .onTapGesture {
                    isQuickOpenPresented = false
                }

            VStack {
                Spacer().frame(height: 120)

                QuickOpenView(onDismiss: {
                    isQuickOpenPresented = false
                })

                Spacer()
            }
        }
        .transition(.opacity)
        .zIndex(100)
    }
}

// MARK: - Notification Handlers (extracted to reduce body complexity)

private struct NotificationHandlers: ViewModifier {
    let sidebarVM: SidebarViewModel
    let todoListVM: TodoListViewModel
    let modelContext: ModelContext
    @Binding var isQuickOpenPresented: Bool
    @Binding var isMoveDialogPresented: Bool
    @Binding var columnVisibility: NavigationSplitViewVisibility

    func body(content: Content) -> some View {
        content
            .onReceive(NotificationCenter.default.publisher(for: .quickFind)) { _ in
                withAnimation(.easeOut(duration: 0.15)) { isQuickOpenPresented = true }
            }
            .onReceive(NotificationCenter.default.publisher(for: .newTodo)) { _ in
                let list = sidebarVM.selectedSmartList
                if list != .trash && list != .logbook { todoListVM.startCreatingTodo() }
            }
            .onReceive(NotificationCenter.default.publisher(for: .newProject)) { _ in
                sidebarVM.isShowingNewProject = true
            }
            .onReceive(NotificationCenter.default.publisher(for: .quickOpenSelected)) { n in
                if let result = n.object as? QuickOpenResult {
                    handleQuickOpenResult(result)
                }
            }
            .onReceive(NotificationCenter.default.publisher(for: .navigateToList)) { n in
                if let list = n.object as? SmartList {
                    withAnimation(.easeInOut(duration: 0.15)) { sidebarVM.selectSmartList(list) }
                }
            }
            .onReceive(NotificationCenter.default.publisher(for: .toggleSidebar)) { _ in
                withAnimation { columnVisibility = columnVisibility == .doubleColumn ? .detailOnly : .doubleColumn }
            }
            .onReceive(NotificationCenter.default.publisher(for: .moveTodo)) { _ in
                isMoveDialogPresented = true
            }
            // Note: setToday, setEvening, setSomeday, setAnytime, completeTodo,
            // cancelTodo, duplicateTodo are handled in TodoListView (has access to todos)
    }

    private func handleQuickOpenResult(_ result: QuickOpenResult) {
        switch result {
        case .todo(let todo):
            todoListVM.selectTodo(todo)
        case .project(let project):
            sidebarVM.selectProject(project)
        case .tag:
            break
        }
    }
}
