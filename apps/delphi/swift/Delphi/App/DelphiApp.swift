import SwiftUI
import SwiftData

@main
struct DelphiApp: App {
    let container: ModelContainer
    @State private var syncSettings = SyncSettings()
    @State private var syncClient: ArkSyncClient

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

        let settings = SyncSettings()
        _syncSettings = State(initialValue: settings)
        _syncClient = State(initialValue: ArkSyncClient(settings: settings))

        QuickEntryPanelController.shared.setup(container: container)
    }

    var body: some Scene {
        WindowGroup {
            ContentView()
                .environment(syncClient)
                .environment(syncSettings)
                .onAppear {
                    syncClient.setModelContainer(container)

                    // Auto-connect on launch if device was previously paired
                    if syncSettings.isPaired {
                        syncClient.connect()
                    }

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
            DelphiCommands()
        }

        Settings {
            SyncSettingsView(settings: syncSettings, syncClient: syncClient)
        }
    }
}
