import SwiftUI

struct NewProjectSheet: View {
    @Bindable var viewModel: SidebarViewModel
    @Environment(\.modelContext) private var modelContext
    @Environment(\.dismiss) private var dismiss

    var body: some View {
        VStack(spacing: 16) {
            Text("Новый проект")
                .font(.headline)

            TextField("Название проекта", text: $viewModel.newProjectTitle)
                .textFieldStyle(.roundedBorder)
                .onSubmit(createProject)

            HStack {
                Button("Отмена") { dismiss() }
                    .keyboardShortcut(.cancelAction)
                Spacer()
                Button("Создать", action: createProject)
                    .keyboardShortcut(.defaultAction)
                    .disabled(viewModel.newProjectTitle.trimmingCharacters(in: .whitespaces).isEmpty)
            }
        }
        .padding(20)
        .frame(width: 300)
    }

    private func createProject() {
        let title = viewModel.newProjectTitle.trimmingCharacters(in: .whitespaces)
        guard !title.isEmpty else { return }
        let project = Project(title: title)
        modelContext.insert(project)
        viewModel.newProjectTitle = ""
        dismiss()
        viewModel.selectProject(project)
    }
}
