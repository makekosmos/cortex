import SwiftUI
import SwiftData

@main
struct DelphiApp: App {
    let container: ModelContainer
    @State private var syncSettings = SyncSettings()
    @State private var syncClient: ArkSyncClient
    @State private var peerManager: ArkPeerManager?

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
        let client = ArkSyncClient(settings: settings)
        _syncSettings = State(initialValue: settings)
        _syncClient = State(initialValue: client)

        QuickEntryPanelController.shared.setup(container: container, syncClient: client)
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
                        startPeerManager()
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

    // MARK: - P2P Peer Manager

    private func startPeerManager() {
        guard syncSettings.isPaired, peerManager == nil else { return }

        let manager = ArkPeerManager(
            meshSecret: syncSettings.apiKey,
            deviceId: syncSettings.deviceId,
            deviceName: Host.current().localizedName ?? "Mac"
        )

        // When peer sends a change, apply it to SwiftData
        let modelContainer = container
        manager.onPeerChange = { change, fromDevice in
            Task { @MainActor in
                let context = modelContainer.mainContext
                applyPeerChange(change, context: context)
                try? context.save()
            }
        }

        manager.start()
        peerManager = manager
    }
}

// MARK: - Apply peer changes to SwiftData

/// Apply a change received from a P2P peer to the local SwiftData store.
@MainActor
private func applyPeerChange(_ change: [String: Any], context: ModelContext) {
    let changeType = change["change_type"] as? String ?? "create"
    let eventData = change["data"] as? [String: Any] ?? change
    let eventType = eventData["event_type"] as? String ?? "task"

    if changeType == "delete" {
        guard let sourceId = change["event_id"] as? String
                ?? (change["data"] as? [String: Any])?["source_id"] as? String,
              let uuid = UUID(uuidString: sourceId)
        else { return }

        switch eventType {
        case "task":
            let descriptor = FetchDescriptor<TodoItem>(predicate: #Predicate { $0.id == uuid })
            if let item = try? context.fetch(descriptor).first {
                context.delete(item)
            }
        case "project":
            let descriptor = FetchDescriptor<Project>(predicate: #Predicate { $0.id == uuid })
            if let item = try? context.fetch(descriptor).first {
                context.delete(item)
            }
        case "area":
            let descriptor = FetchDescriptor<Area>(predicate: #Predicate { $0.id == uuid })
            if let item = try? context.fetch(descriptor).first {
                context.delete(item)
            }
        case "tag":
            let descriptor = FetchDescriptor<Tag>(predicate: #Predicate { $0.id == uuid })
            if let item = try? context.fetch(descriptor).first {
                context.delete(item)
            }
        default:
            break
        }
        return
    }

    switch eventType {
    case "task":
        _ = ArkEventMapper.arkEventToTodo(eventData, context: context)
    case "project":
        _ = ArkEventMapper.arkEventToProject(eventData, context: context)
    case "area":
        _ = ArkEventMapper.arkEventToArea(eventData, context: context)
    case "tag":
        _ = ArkEventMapper.arkEventToTag(eventData, context: context)
    default:
        break
    }
}
