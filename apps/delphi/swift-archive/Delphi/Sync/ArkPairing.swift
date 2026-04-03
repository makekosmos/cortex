import Foundation
import os

struct PairingResult: Codable {
    let serverUrl: String
    let apiKey: String
    let deviceId: String

    enum CodingKeys: String, CodingKey {
        case serverUrl = "server_url"
        case apiKey = "api_key"
        case deviceId = "device_id"
    }
}

enum PairingError: LocalizedError {
    case invalidURL
    case invalidCode
    case serverError(String)
    case networkError(Error)

    var errorDescription: String? {
        switch self {
        case .invalidURL:
            "Некорректный URL сервера"
        case .invalidCode:
            "Некорректный код сопряжения. Формат: ark-XXXX"
        case .serverError(let message):
            "Ошибка сервера: \(message)"
        case .networkError(let error):
            "Ошибка сети: \(error.localizedDescription)"
        }
    }
}

struct ArkPairing {
    private static let logger = Logger(subsystem: "com.kosmos.delphi", category: "ArkPairing")

    /// Claim a pairing code from the Ark server.
    /// - Parameters:
    ///   - serverUrl: Base URL of the Ark server (e.g. "http://localhost:8000")
    ///   - code: Pairing code from `ark pair` CLI (e.g. "ark-7f3k")
    ///   - deviceName: Human-readable device name
    /// - Returns: PairingResult with credentials
    static func claim(serverUrl: String, code: String, deviceName: String) async throws -> PairingResult {
        let trimmedCode = code.trimmingCharacters(in: .whitespacesAndNewlines).lowercased()

        // Validate code format: "ark-XXXX" (8 chars total)
        guard trimmedCode.count == 8,
              trimmedCode.hasPrefix("ark-") else {
            throw PairingError.invalidCode
        }

        let baseUrl = serverUrl.trimmingCharacters(in: .whitespacesAndNewlines)
            .trimmingCharacters(in: CharacterSet(charactersIn: "/"))

        guard let url = URL(string: "\(baseUrl)/pairing/claim") else {
            throw PairingError.invalidURL
        }

        var request = URLRequest(url: url)
        request.httpMethod = "POST"
        request.setValue("application/json", forHTTPHeaderField: "Content-Type")
        request.timeoutInterval = 10

        let body: [String: String] = [
            "code": trimmedCode,
            "device_name": deviceName,
            "platform": "macos",
        ]
        request.httpBody = try JSONEncoder().encode(body)

        logger.info("Claiming pairing code at \(url.absoluteString)")

        let data: Data
        let response: URLResponse
        do {
            (data, response) = try await URLSession.shared.data(for: request)
        } catch {
            throw PairingError.networkError(error)
        }

        guard let httpResponse = response as? HTTPURLResponse else {
            throw PairingError.networkError(URLError(.badServerResponse))
        }

        if httpResponse.statusCode != 200 {
            // Try to extract error message from response body
            if let errorJson = try? JSONSerialization.jsonObject(with: data) as? [String: Any],
               let message = errorJson["detail"] as? String ?? errorJson["message"] as? String {
                throw PairingError.serverError(message)
            }
            throw PairingError.serverError("HTTP \(httpResponse.statusCode)")
        }

        let result = try JSONDecoder().decode(PairingResult.self, from: data)
        logger.info("Pairing successful, device_id=\(result.deviceId)")
        return result
    }
}
