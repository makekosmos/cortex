import SwiftUI

struct SyncSettingsView: View {
    @Bindable var settings: SyncSettings
    var syncClient: ArkSyncClient

    @State private var discovery = ArkDiscovery()
    @State private var pairingCode = ""
    @State private var serverUrlInput = ""
    @State private var isPairing = false
    @State private var pairingError: String?
    @State private var pairingSuccess = false

    var body: some View {
        Form {
            if settings.isConfigured {
                pairedSection
            } else {
                pairingSection
            }

            connectionSection

            if settings.isConfigured {
                actionsSection
            }

            deviceSection
        }
        .formStyle(.grouped)
        .frame(minWidth: 400)
        .onAppear {
            serverUrlInput = settings.serverUrl
            discovery.startSearching()
            Task {
                if let url = await discovery.checkLocalhost() {
                    if serverUrlInput.isEmpty {
                        serverUrlInput = url
                    }
                }
            }
        }
        .onChange(of: discovery.discoveredUrl) {
            if let url = discovery.discoveredUrl, serverUrlInput.isEmpty {
                serverUrlInput = url
            }
        }
    }

    // MARK: - Pairing (not yet paired)

    private var pairingSection: some View {
        Section {
            VStack(alignment: .leading, spacing: 12) {
                Text("Подключение к Ark")
                    .font(.headline)

                Text("Запустите `ark pair` на сервере, затем введите полученный код.")
                    .foregroundStyle(.secondary)
                    .font(.callout)

                HStack {
                    TextField("URL сервера", text: $serverUrlInput)
                        .textFieldStyle(.roundedBorder)

                    if discovery.isSearching {
                        ProgressView()
                            .controlSize(.small)
                            .help("Поиск Ark в локальной сети...")
                    } else if discovery.discoveredUrl != nil {
                        Image(systemName: "checkmark.circle.fill")
                            .foregroundStyle(.green)
                            .help("Обнаружен в локальной сети")
                    }

                    Button {
                        discovery.startSearching()
                        Task {
                            if let url = await discovery.checkLocalhost() {
                                serverUrlInput = url
                            }
                        }
                    } label: {
                        Image(systemName: "arrow.clockwise")
                    }
                    .buttonStyle(.borderless)
                    .help("Искать Ark в локальной сети")
                }

                HStack {
                    TextField("Код сопряжения", text: $pairingCode, prompt: Text("ark-XXXX"))
                        .textFieldStyle(.roundedBorder)
                        .textInputAutocapitalization(.never)
                        .onSubmit { startPairing() }

                    Button("Подключить") {
                        startPairing()
                    }
                    .disabled(!canPair)
                    .buttonStyle(.borderedProminent)
                }

                if isPairing {
                    HStack {
                        ProgressView()
                            .controlSize(.small)
                        Text("Сопряжение...")
                            .foregroundStyle(.secondary)
                    }
                }

                if let error = pairingError {
                    Label(error, systemImage: "exclamationmark.triangle.fill")
                        .foregroundStyle(.red)
                        .font(.callout)
                }

                if pairingSuccess {
                    Label("Подключено!", systemImage: "checkmark.circle.fill")
                        .foregroundStyle(.green)
                        .font(.callout)
                }
            }
        }
    }

    // MARK: - Paired state

    private var pairedSection: some View {
        Section("Ark сервер") {
            LabeledContent("Сервер", value: settings.serverUrl)
                .textSelection(.enabled)

            LabeledContent("API Key") {
                Text(maskedApiKey)
                    .foregroundStyle(.secondary)
                    .textSelection(.enabled)
            }

            Button("Отвязать устройство", role: .destructive) {
                unpair()
            }
        }
    }

    // MARK: - Connection status

    private var connectionSection: some View {
        Section("Подключение") {
            Toggle("Автосинхронизация", isOn: $settings.isAutoSyncEnabled)

            HStack {
                Circle()
                    .fill(syncClient.isConnected ? .green : .red)
                    .frame(width: 8, height: 8)

                Text(syncClient.isConnected ? "Подключено" : "Отключено")
                    .foregroundStyle(.secondary)

                Spacer()

                if syncClient.isSyncing {
                    ProgressView()
                        .controlSize(.small)
                    Text("Синхронизация...")
                        .foregroundStyle(.secondary)
                        .font(.caption)
                }
            }

            if syncClient.pendingChanges > 0 {
                HStack {
                    Image(systemName: "arrow.up.circle")
                        .foregroundStyle(.orange)
                    Text("\(syncClient.pendingChanges) изменений в очереди")
                        .foregroundStyle(.secondary)
                }
            }

            if let lastSync = syncClient.lastSyncAt {
                HStack {
                    Image(systemName: "clock")
                        .foregroundStyle(.secondary)
                    Text("Последняя синхронизация: \(lastSync.formatted(.relative(presentation: .named)))")
                        .foregroundStyle(.secondary)
                        .font(.caption)
                }
            }
        }
    }

    // MARK: - Actions

    private var actionsSection: some View {
        Section {
            HStack {
                if syncClient.isConnected {
                    Button("Отключиться") {
                        syncClient.disconnect()
                    }
                } else {
                    Button("Подключиться") {
                        syncClient.connect()
                    }
                    .disabled(!settings.isConfigured)
                }

                Spacer()

                Button("Синхронизировать") {
                    syncClient.syncNow()
                }
                .disabled(!syncClient.isConnected)
            }
        }
    }

    // MARK: - Device info

    private var deviceSection: some View {
        Section {
            LabeledContent("Device ID", value: settings.deviceId)
                .foregroundStyle(.secondary)
                .font(.caption)
                .textSelection(.enabled)
        }
    }

    // MARK: - Helpers

    private var canPair: Bool {
        !serverUrlInput.trimmingCharacters(in: .whitespacesAndNewlines).isEmpty
        && !pairingCode.trimmingCharacters(in: .whitespacesAndNewlines).isEmpty
        && !isPairing
    }

    private var maskedApiKey: String {
        let key = settings.apiKey
        if key.count <= 8 { return String(repeating: "*", count: key.count) }
        return String(key.prefix(4)) + "..." + String(key.suffix(4))
    }

    private func startPairing() {
        guard canPair else { return }

        isPairing = true
        pairingError = nil
        pairingSuccess = false

        let url = serverUrlInput.trimmingCharacters(in: .whitespacesAndNewlines)
        let code = pairingCode.trimmingCharacters(in: .whitespacesAndNewlines)
        let deviceName = Host.current().localizedName ?? "Mac"

        Task {
            do {
                let result = try await ArkPairing.claim(
                    serverUrl: url,
                    code: code,
                    deviceName: deviceName
                )

                settings.serverUrl = url
                settings.apiKey = result.apiKey
                pairingSuccess = true
                pairingCode = ""

                // Auto-connect after successful pairing
                settings.isAutoSyncEnabled = true
                syncClient.connect()
            } catch {
                pairingError = error.localizedDescription
            }

            isPairing = false
        }
    }

    private func unpair() {
        syncClient.disconnect()
        settings.serverUrl = ""
        settings.apiKey = ""
        settings.isAutoSyncEnabled = false
        serverUrlInput = ""
        pairingCode = ""
        pairingSuccess = false
        pairingError = nil
    }
}
