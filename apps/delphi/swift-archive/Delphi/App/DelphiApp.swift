import SwiftUI
import SwiftData

/// Notification posted when the user leaves a space (from Settings or elsewhere).
extension Notification.Name {
    static let didLeaveSpace = Notification.Name("com.kosmos.delphi.didLeaveSpace")
}

@main
struct DelphiApp: App {
    let container: ModelContainer
    @State private var syncSettings = SyncSettings()
    @State private var syncClient: ArkSyncClient
    @State private var peerManager: PeerManager?
    @State private var spaceManager = SpaceManager()
    @State private var showSpaceSetup = false

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

                    if let spaceCode = spaceManager.activeSpaceCode {
                        // P2P mode: start PeerManager with full sync
                        startPeerManager(spaceCode: spaceCode)
                    } else {
                        // No space configured — show setup
                        showSpaceSetup = true
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
                .onReceive(NotificationCenter.default.publisher(for: .didLeaveSpace)) { _ in
                    leaveCurrentSpace()
                }
                .sheet(isPresented: $showSpaceSetup) {
                    SpaceSetupView { code, peerAddresses in
                        spaceManager.activeSpaceCode = code
                        spaceManager.addSavedSpaceCode(code)
                        syncSettings.spaceCode = code
                        showSpaceSetup = false
                        startPeerManager(spaceCode: code, initialPeerAddresses: peerAddresses)
                    }
                    .frame(width: 440, height: 520)
                }
        }
        .modelContainer(container)
        .windowToolbarStyle(.unified(showsTitle: false))
        .defaultSize(width: 1000, height: 650)
        .commands {
            DelphiCommands()
        }

        Settings {
            SyncSettingsView(settings: syncSettings, syncClient: syncClient, peerManager: peerManager)
        }
    }

    // MARK: - Leave Space

    private func leaveCurrentSpace() {
        // Stop P2P sync
        peerManager?.stop()
        peerManager = nil

        // Clear space settings
        spaceManager.clearActiveSpace()
        syncSettings.spaceCode = nil

        // Clear all tasks/projects from SwiftData so they don't linger in the UI
        let context = container.mainContext
        do {
            let todos = try context.fetch(FetchDescriptor<TodoItem>())
            for item in todos { context.delete(item) }

            let projects = try context.fetch(FetchDescriptor<Project>())
            for item in projects { context.delete(item) }

            let areas = try context.fetch(FetchDescriptor<Area>())
            for item in areas { context.delete(item) }

            let tags = try context.fetch(FetchDescriptor<Tag>())
            for item in tags { context.delete(item) }

            let headings = try context.fetch(FetchDescriptor<Heading>())
            for item in headings { context.delete(item) }

            try context.save()
        } catch {
            print("Failed to clear data on leave space: \(error)")
        }

        // Reset sync state
        syncSettings.resetVector()
        syncSettings.serverEpoch = nil

        // Show setup screen
        showSpaceSetup = true
    }

    // MARK: - PeerManager Setup

    private func startPeerManager(spaceCode: String, initialPeerAddresses: [String] = []) {
        guard peerManager == nil else { return }

        let manager = PeerManager(
            deviceId: syncSettings.deviceId,
            deviceName: Host.current().localizedName ?? "Mac"
        )
        manager.setModelContainer(container)

        // If we have initial addresses from QR/code join, add a temporary peer
        if !initialPeerAddresses.isEmpty {
            let tempPeer = PeerRecord(
                device_id: "pending-\(generateSyncId())",
                device_name: "Unknown Peer",
                addresses: initialPeerAddresses,
                last_seen: ISO8601DateFormatter().string(from: Date())
            )
            manager.addPeerAndConnect(tempPeer)
        }

        manager.start(spaceCode: spaceCode)
        peerManager = manager
    }
}
