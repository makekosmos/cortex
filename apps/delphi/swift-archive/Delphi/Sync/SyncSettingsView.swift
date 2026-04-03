import SwiftUI
import CoreImage.CIFilterBuiltins

struct SyncSettingsView: View {
    @Bindable var settings: SyncSettings
    var syncClient: ArkSyncClient
    var peerManager: PeerManager?

    @State private var connectionCode = ""
    @State private var connectionError: String?
    @State private var connectionSuccess = false
    @State private var showLegacySync = false

    private let spaceManager = SpaceManager()

    var body: some View {
        Form {
            // Space section — always visible when space is configured
            if settings.isSpaceConfigured {
                spaceSection
                peersSection
                qrSection
                leaveSpaceSection
            }

            // Ark WS relay — legacy, hidden by default
            if showLegacySync {
                if settings.isConfigured {
                    pairedSection
                } else {
                    pairingSection
                }

                connectionSection

                if settings.isConfigured {
                    actionsSection
                }
            }

            deviceSection

            Section {
                Button(showLegacySync ? "Скрыть Ark WS (legacy)" : "Показать Ark WS (legacy)") {
                    showLegacySync.toggle()
                }
                .foregroundStyle(.secondary)
                .font(.caption)
            }
        }
        .formStyle(.grouped)
        .frame(minWidth: 400)
    }

    // MARK: - Space

    private var spaceSection: some View {
        Section("Пространство") {
            if let code = settings.spaceCode {
                LabeledContent("Код пространства") {
                    Text(spaceManager.formatCode(code))
                        .font(.system(.body, design: .monospaced))
                        .textSelection(.enabled)
                }
            }
        }
    }

    // MARK: - Connected Peers

    private var peersSection: some View {
        Section("Устройства") {
            if let pm = peerManager {
                HStack {
                    Circle()
                        .fill(pm.connectedPeerCount > 0 ? .green : .orange)
                        .frame(width: 8, height: 8)
                    Text(pm.connectedPeerCount > 0
                        ? "Подключено: \(pm.connectedPeerCount)"
                        : "Нет подключённых устройств")
                        .foregroundStyle(.secondary)
                }

                if !pm.connectedPeerNames.isEmpty {
                    ForEach(pm.connectedPeerNames, id: \.self) { name in
                        HStack {
                            Image(systemName: "desktopcomputer")
                                .foregroundStyle(.secondary)
                            Text(name)
                        }
                    }
                }
            } else {
                Text("Синхронизация не запущена")
                    .foregroundStyle(.secondary)
            }
        }
    }

    // MARK: - QR Code

    private var qrSection: some View {
        Section("Поделиться") {
            if let code = settings.spaceCode {
                let addresses = getOwnAddresses()
                let qrPayload = spaceManager.generateQrPayload(code: code, addresses: addresses)

                if let qrImage = generateQRCode(from: qrPayload) {
                    HStack {
                        Spacer()
                        Image(nsImage: qrImage)
                            .interpolation(.none)
                            .resizable()
                            .scaledToFit()
                            .frame(width: 160, height: 160)
                            .cornerRadius(8)
                        Spacer()
                    }
                }

                Text("Отсканируйте QR на другом устройстве для присоединения к пространству.")
                    .font(.caption)
                    .foregroundStyle(.secondary)
            }
        }
    }

    // MARK: - Leave Space

    private var leaveSpaceSection: some View {
        Section {
            Button("Покинуть пространство", role: .destructive) {
                NotificationCenter.default.post(name: .didLeaveSpace, object: nil)
            }
        }
    }

    // MARK: - Pairing (not yet paired) — legacy

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

    // MARK: - Paired state — legacy

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

    // MARK: - Connection status — legacy

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

    // MARK: - Actions — legacy

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

    private var maskedApiKey: String {
        let key = settings.apiKey
        if key.count <= 8 { return String(repeating: "*", count: key.count) }
        return String(key.prefix(4)) + "..." + String(key.suffix(4))
    }

    /// Parse ark://host:port?key=SECRET -> (serverUrl, apiKey)
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

    // MARK: - QR Code Generation

    private func generateQRCode(from string: String) -> NSImage? {
        let context = CIContext()
        let filter = CIFilter.qrCodeGenerator()
        filter.message = Data(string.utf8)
        filter.correctionLevel = "M"

        guard let outputImage = filter.outputImage else { return nil }

        let scale = 8.0
        let scaled = outputImage.transformed(by: CGAffineTransform(scaleX: scale, y: scale))

        guard let cgImage = context.createCGImage(scaled, from: scaled.extent) else { return nil }
        return NSImage(cgImage: cgImage, size: NSSize(width: scaled.extent.width, height: scaled.extent.height))
    }
}
