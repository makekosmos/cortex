import SwiftUI

struct SyncSettingsView: View {
    @Bindable var settings: SyncSettings
    var syncClient: ArkSyncClient

    @State private var showApiKey = false

    var body: some View {
        Form {
            Section("Сервер Ark") {
                TextField("URL сервера", text: $settings.serverUrl)
                    .textFieldStyle(.roundedBorder)
                    .help("Например: http://your-vps:8000")

                HStack {
                    if showApiKey {
                        TextField("API Key", text: $settings.apiKey)
                            .textFieldStyle(.roundedBorder)
                    } else {
                        SecureField("API Key", text: $settings.apiKey)
                            .textFieldStyle(.roundedBorder)
                    }
                    Button {
                        showApiKey.toggle()
                    } label: {
                        Image(systemName: showApiKey ? "eye.slash" : "eye")
                    }
                    .buttonStyle(.borderless)
                }
            }

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

            Section {
                LabeledContent("Device ID", value: settings.deviceId)
                    .foregroundStyle(.secondary)
                    .font(.caption)
                    .textSelection(.enabled)
            }
        }
        .formStyle(.grouped)
        .frame(minWidth: 400)
    }
}
