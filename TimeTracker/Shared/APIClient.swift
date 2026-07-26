import Foundation
import Security

enum APIError: LocalizedError {
    case notConfigured
    case badURL
    case server(status: Int, message: String)
    case decoding
    case changedDuringRefresh

    var errorDescription: String? {
        switch self {
        case .notConfigured:
            return "Server URL or access key not configured. Open Settings to connect."
        case .badURL:
            return "The server URL is invalid."
        case .server(let status, let message):
            return message.isEmpty ? "Server error (\(status))" : message
        case .decoding:
            return "Unexpected response from server."
        case .changedDuringRefresh:
            return "TimeTracker changed while refreshing. Try again."
        }
    }
}

nonisolated struct ServerConfig: Sendable {
    var baseURL: URL
    var accessKey: String

    static let appGroupID = "group.at.janez.TimeTracker"
    static var sharedDefaults: UserDefaults? { UserDefaults(suiteName: appGroupID) }

    static func load() -> ServerConfig? {
        guard let raw = sharedDefaults?.string(forKey: "serverURL"),
              let url = URL(string: raw),
              let key = Keychain.read("accessKey"),
              !key.isEmpty
        else { return nil }
        return ServerConfig(baseURL: url, accessKey: key)
    }
}

nonisolated enum Keychain {
    private static var accessGroup: String? {
        Bundle.main.object(
            forInfoDictionaryKey: "TimeTrackerKeychainAccessGroup"
        ) as? String
    }

    private static func query(_ account: String) throws -> [String: Any] {
        guard let accessGroup, !accessGroup.isEmpty else {
            throw KeychainError.missingAccessGroup
        }
        return [
            kSecClass as String: kSecClassGenericPassword,
            kSecAttrService as String: "at.janez.TimeTracker",
            kSecAttrAccount as String: account,
            kSecAttrAccessGroup as String: accessGroup,
        ]
    }

    static func save(_ value: String, for account: String) throws {
        let item = try query(account)
        let valueData = Data(value.utf8)
        var status = SecItemUpdate(
            item as CFDictionary,
            [kSecValueData as String: valueData] as CFDictionary
        )
        if status == errSecItemNotFound {
            var newItem = item
            newItem[kSecValueData as String] = valueData
            status = SecItemAdd(newItem as CFDictionary, nil)
        }
        guard status == errSecSuccess else {
            throw KeychainError.writeFailed(status)
        }
    }

    static func read(_ account: String) -> String? {
        guard var item = try? query(account) else { return nil }
        item[kSecReturnData as String] = true
        item[kSecMatchLimit as String] = kSecMatchLimitOne
        var result: AnyObject?
        guard SecItemCopyMatching(item as CFDictionary, &result) == errSecSuccess,
              let data = result as? Data
        else {
            return nil
        }
        return String(data: data, encoding: .utf8)
    }
}

enum KeychainError: LocalizedError {
    case missingAccessGroup
    case writeFailed(OSStatus)

    var errorDescription: String? {
        switch self {
        case .missingAccessGroup:
            return "The shared Keychain access group is not configured."
        case .writeFailed(let status):
            return "Could not save the access key (Keychain error \(status))."
        }
    }
}

struct APIClient: Sendable {
    let config: ServerConfig

    private func url(_ path: String, query: [URLQueryItem] = []) throws -> URL {
        guard var components = URLComponents(
            url: config.baseURL.appendingPathComponent(path),
            resolvingAgainstBaseURL: false
        ) else { throw APIError.badURL }

        // The server authenticates on the `key` query parameter, so it has to
        // survive alongside anything the user already put in the server URL.
        var items = components.queryItems ?? []
        items.append(contentsOf: query)
        items.removeAll { $0.name == "key" }
        items.append(URLQueryItem(name: "key", value: config.accessKey))

        components.queryItems = items
        guard let url = components.url else { throw APIError.badURL }
        return url
    }

    private func send<T: Decodable>(
        _ path: String,
        method: String = "GET",
        query: [URLQueryItem] = [],
        body: Encodable? = nil
    ) async throws -> T {
        var request = URLRequest(url: try url(path, query: query))
        request.httpMethod = method
        request.timeoutInterval = 15
        request.setValue("application/json", forHTTPHeaderField: "Accept")
        if let body {
            request.setValue("application/json", forHTTPHeaderField: "Content-Type")
            request.httpBody = try JSONEncoder().encode(body)
        }
        let (data, response) = try await URLSession.shared.data(for: request)
        guard let http = response as? HTTPURLResponse else { throw APIError.decoding }
        guard http.statusCode == 200 else {
            throw APIError.server(
                status: http.statusCode,
                message: String(data: data, encoding: .utf8) ?? ""
            )
        }
        do {
            return try JSONDecoder().decode(T.self, from: data)
        } catch {
            throw APIError.decoding
        }
    }

    struct EntryPayload: Encodable {
        var new_state: Int?
        var start_timestamp: Int64?
        var force: Bool?
    }

    struct EntryResponse: Decodable {
        let new_state: Int
        let start_timestamp: Int64
    }

    func addEntry(stateID: Int, startTimestamp: Int64, force: Bool) async throws -> EntryResponse {
        try await send(
            "api/entry",
            method: "POST",
            body: EntryPayload(new_state: stateID, start_timestamp: startTimestamp, force: force)
        )
    }

    func updateEntry(
        index: UInt64,
        stateID: Int?,
        startTimestamp: Int64?,
        force: Bool
    ) async throws -> EntryResponse {
        try await send(
            "api/entry/\(index)",
            method: "PUT",
            body: EntryPayload(new_state: stateID, start_timestamp: startTimestamp, force: force)
        )
    }

    func length() async throws -> UInt64 {
        try await send("api/length")
    }

    func summary(days: Int) async throws -> [Int64] {
        try await send("api/data", query: [.init(name: "days", value: String(days))])
    }

    func recents(count: Int, days: Int) async throws -> [[Int64]] {
        try await send("api/recents", query: [
            .init(name: "count", value: String(count)),
            .init(name: "days", value: String(days)),
        ])
    }
}
