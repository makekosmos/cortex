import SwiftUI
import SwiftData

@main
struct TaskoApp: App {
    let container: ModelContainer

    init() {
        let schema = Schema([
            TodoItem.self,
            Project.self,
            Area.self,
            Tag.self,
            Heading.self,
            ChecklistItem.self
        ])
        let config = ModelConfiguration(schema: schema)

        do {
            container = try ModelContainer(for: schema, configurations: [config])
        } catch {
            print("SwiftData migration failed: \(error). Recreating store...")
            let url = config.url
            for suffix in ["", "-wal", "-shm"] {
                try? FileManager.default.removeItem(atPath: url.path() + suffix)
            }
            do {
                container = try ModelContainer(for: schema, configurations: [config])
            } catch {
                fatalError("ModelContainer creation failed: \(error)")
            }
        }

        QuickEntryPanelController.shared.setup(container: container)
    }

    var body: some Scene {
        WindowGroup {
            ContentView()
                .onAppear {
                    // Register local hotkey for Quick Entry (Ctrl+Space) after app is ready
                    NSEvent.addLocalMonitorForEvents(matching: .keyDown) { event in
                        if event.modifierFlags.contains(.control) && event.keyCode == 49 {
                            QuickEntryPanelController.shared.toggle()
                            return nil
                        }
                        return event
                    }
                }
        }
        .modelContainer(container)
        .windowToolbarStyle(.unified(showsTitle: false))
        .defaultSize(width: 1000, height: 650)
        .commands {
            TaskoCommands()
        }
    }
}
