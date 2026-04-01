import SwiftUI
import CoreImage.CIFilterBuiltins

/// Full-screen overlay shown when no Ark Space is configured.
/// Allows creating a new space (with QR) or joining an existing one via code + optional IP.
struct SpaceSetupView: View {
    let onSpaceJoined: (String, [String]) -> Void  // (code, peerAddresses)

    @State private var mode: Mode = .choose
    @State private var generatedCode = ""
    @State private var joinInput = ""
    @State private var joinError = ""

    private let spaceManager = SpaceManager()

    enum Mode { case choose, create, join }

    var body: some View {
        ZStack {
            Color(NSColor.windowBackgroundColor).ignoresSafeArea()

            VStack(spacing: 0) {
                switch mode {
                case .choose:
                    chooseView
                case .create:
                    createView
                case .join:
                    joinView
                }
            }
            .frame(width: 380)
            .padding(28)
            .background(Color(NSColor.controlBackgroundColor))
            .cornerRadius(14)
            .shadow(color: .black.opacity(0.18), radius: 24, x: 0, y: 8)
        }
    }

    // MARK: - Choose mode

    private var chooseView: some View {
        VStack(alignment: .leading, spacing: 14) {
            Text("Ark Space")
                .font(.title2.bold())
            Text("Синхронизация без сервера — через локальную сеть. Создайте пространство или присоединитесь к существующему.")
                .font(.callout)
                .foregroundStyle(.secondary)
                .fixedSize(horizontal: false, vertical: true)

            VStack(spacing: 8) {
                Button {
                    generatedCode = spaceManager.generateSpaceCode()
                    mode = .create
                } label: {
                    Text("Создать пространство")
                        .frame(maxWidth: .infinity)
                }
                .buttonStyle(.borderedProminent)
                .controlSize(.large)

                Button {
                    mode = .join
                } label: {
                    Text("Присоединиться")
                        .frame(maxWidth: .infinity)
                }
                .buttonStyle(.bordered)
                .controlSize(.large)
            }
        }
    }

    // MARK: - Create mode

    private var createView: some View {
        VStack(alignment: .leading, spacing: 14) {
            Button("< Назад") { mode = .choose }
                .buttonStyle(.plain)
                .font(.caption)
                .foregroundStyle(.secondary)

            Text("Ваш код пространства")
                .font(.title2.bold())
            Text("Поделитесь этим кодом или QR с другими устройствами для синхронизации.")
                .font(.callout)
                .foregroundStyle(.secondary)
                .fixedSize(horizontal: false, vertical: true)

            Text(generatedCode)
                .font(.system(.title, design: .monospaced).weight(.medium))
                .frame(maxWidth: .infinity)
                .padding(16)
                .background(Color(NSColor.controlColor))
                .cornerRadius(10)
                .textSelection(.enabled)

            // QR code
            let normalizedCode = spaceManager.normalizeCode(generatedCode)
            let addresses = getOwnAddresses()
            let qrPayload = spaceManager.generateQrPayload(code: normalizedCode, addresses: addresses)

            if let qrImage = generateQRCode(from: qrPayload) {
                HStack {
                    Spacer()
                    Image(nsImage: qrImage)
                        .interpolation(.none)
                        .resizable()
                        .scaledToFit()
                        .frame(width: 180, height: 180)
                        .cornerRadius(8)
                    Spacer()
                }
            }

            Button {
                onSpaceJoined(normalizedCode, [])
            } label: {
                Text("Начать использование")
                    .frame(maxWidth: .infinity)
            }
            .buttonStyle(.borderedProminent)
            .controlSize(.large)
        }
    }

    // MARK: - Join mode

    private var joinView: some View {
        VStack(alignment: .leading, spacing: 14) {
            Button("< Назад") { mode = .choose }
                .buttonStyle(.plain)
                .font(.caption)
                .foregroundStyle(.secondary)

            Text("Присоединиться")
                .font(.title2.bold())
            Text("Скопируйте ссылку из QR-кода на другом устройстве и вставьте сюда.")
                .font(.callout)
                .foregroundStyle(.secondary)

            TextField("ark://join?code=...&addrs=...", text: $joinInput)
                .textFieldStyle(.roundedBorder)
                .font(.system(.body, design: .monospaced))
                .onChange(of: joinInput) { _, newValue in
                    if let parsed = spaceManager.parseQrPayload(newValue) {
                        joinError = ""
                        onSpaceJoined(parsed.code, parsed.addresses)
                    }
                }
                .onSubmit { attemptJoin() }

            if !joinError.isEmpty {
                Text(joinError)
                    .font(.caption)
                    .foregroundStyle(.red)
            }

            Button {
                attemptJoin()
            } label: {
                Text("Присоединиться")
                    .frame(maxWidth: .infinity)
            }
            .buttonStyle(.borderedProminent)
            .controlSize(.large)
        }
    }

    // MARK: - Actions

    private func attemptJoin() {
        if let parsed = spaceManager.parseQrPayload(joinInput) {
            joinError = ""
            onSpaceJoined(parsed.code, parsed.addresses)
            return
        }

        joinError = "Вставьте ссылку из QR-кода (ark://join?...)"
    }

    // MARK: - QR Code Generation

    private func generateQRCode(from string: String) -> NSImage? {
        let context = CIContext()
        let filter = CIFilter.qrCodeGenerator()
        filter.message = Data(string.utf8)
        filter.correctionLevel = "M"

        guard let outputImage = filter.outputImage else { return nil }

        // Scale up for crisp rendering
        let scale = 10.0
        let scaled = outputImage.transformed(by: CGAffineTransform(scaleX: scale, y: scale))

        guard let cgImage = context.createCGImage(scaled, from: scaled.extent) else { return nil }
        return NSImage(cgImage: cgImage, size: NSSize(width: scaled.extent.width, height: scaled.extent.height))
    }
}
