import SwiftUI

struct SyncSettingsView: View {
    @Bindable var settings: SyncSettings
    var syncClient: ArkSyncClient

    @State private var connectionCode = ""
    @State private var connectionError: String?
    @State private var connectionSuccess = false

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

            clearDataSection

            deviceSection
        }
        .formStyle(.grouped)
        .frame(minWidth: 400)
    }

    // MARK: - Pairing (not yet paired)

    private var pairingSection: some View {
        Section {
            VStack(alignment: .leading, spacing: 12) {
                Text("Подключение к Ark")
                    .font(.headline)

                Text("Вставьте строку подключения, полученную от Ark сервера.")
                    .foregroundStyle(.secondary)
                    .font(.callout)

                HStack {
                    TextField(
                        "Код подключения",
                        text: $connectionCode,
                        prompt: Text("ark://192.168.1.5:8000?key=...")
                    )
                    .textFieldStyle(.roundedBorder)
                    .onSubmit { connect() }

                    Button("Подключить") {
                        connect()
                    }
                    .disabled(connectionCode.trimmingCharacters(in: .whitespacesAndNewlines).isEmpty)
                    .buttonStyle(.borderedProminent)
                }

                if let error = connectionError {
                    Label(error, systemImage: "exclamationmark.triangle.fill")
                        .foregroundStyle(.red)
                        .font(.callout)
                }

                if connectionSuccess {
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

    // MARK: - Clear data (always visible)

    private var clearDataSection: some View {
        Section {
            Button("Очистить данные", role: .destructive) {
                syncClient.clearLocalData()
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

    private var maskedApiKey: String {
        let key = settings.apiKey
        if key.count <= 8 { return String(repeating: "*", count: key.count) }
        return String(key.prefix(4)) + "..." + String(key.suffix(4))
    }

    /// Parse ark://host:port?key=SECRET → (serverUrl, apiKey)
    private func parseConnectionString(_ input: String) -> (serverUrl: String, apiKey: String)? {
        let trimmed = input.trimmingCharacters(in: .whitespacesAndNewlines)
        guard trimmed.hasPrefix("ark://") else { return nil }

        let rest = String(trimmed.dropFirst("ark://".count))
        guard let keyRange = rest.range(of: "?key=") else { return nil }

        let hostPart = String(rest[rest.startIndex..<keyRange.lowerBound])
        let apiKey = String(rest[keyRange.upperBound...])

        guard !hostPart.isEmpty, !apiKey.isEmpty else { return nil }

        return (serverUrl: "http://\(hostPart)", apiKey: apiKey)
    }

    private func connect() {
        connectionError = nil
        connectionSuccess = false

        guard let parsed = parseConnectionString(connectionCode) else {
            connectionError = "Неверный формат. Ожидается: ark://host:port?key=..."
            return
        }

        settings.serverUrl = parsed.serverUrl
        settings.apiKey = parsed.apiKey
        settings.isAutoSyncEnabled = true
        connectionSuccess = true
        connectionCode = ""

        syncClient.connect()
    }

    private func unpair() {
        syncClient.disconnect()
        settings.serverUrl = ""
        settings.apiKey = ""
        settings.isAutoSyncEnabled = false
        connectionCode = ""
        connectionSuccess = false
        connectionError = nil
    }
}
