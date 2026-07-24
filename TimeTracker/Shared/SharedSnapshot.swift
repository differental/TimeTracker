import Foundation

nonisolated struct SharedTrackerSnapshot: Codable, Equatable, Sendable {
    static let empty = SharedTrackerSnapshot(
        updatedAt: .distantPast,
        currentStateID: nil,
        currentStartTimestamp: nil,
        todayTotals: Array(repeating: 0, count: TrackerState.all.count),
        predictedStateIDs: Array(TrackerState.all.prefix(3).map(\.id))
    )

    var updatedAt: Date
    var currentStateID: Int?
    var currentStartTimestamp: Int64?
    var todayTotals: [Int64]
    var predictedStateIDs: [Int]

    var currentStart: Date? {
        currentStartTimestamp.map {
            Date(timeIntervalSince1970: Double($0) / 1_000)
        }
    }

    var isAvailable: Bool {
        updatedAt != .distantPast
    }
}

enum SharedSnapshotStore {
    nonisolated private static let fileName = "tracker-snapshot.json"

    nonisolated static func load() -> SharedTrackerSnapshot {
        guard let url = snapshotURL(),
              let data = try? Data(contentsOf: url),
              let snapshot = try? JSONDecoder().decode(
                SharedTrackerSnapshot.self,
                from: data
              )
        else {
            return .empty
        }
        return snapshot
    }

    nonisolated static func save(_ snapshot: SharedTrackerSnapshot) throws {
        guard let url = snapshotURL() else {
            throw SharedSnapshotError.missingAppGroup
        }
        let data = try JSONEncoder().encode(snapshot)
        try data.write(to: url, options: [.atomic, .completeFileProtectionUntilFirstUserAuthentication])
    }

    nonisolated private static func snapshotURL() -> URL? {
        FileManager.default
            .containerURL(
                forSecurityApplicationGroupIdentifier: ServerConfig.appGroupID
            )?
            .appendingPathComponent(fileName, isDirectory: false)
    }
}

enum SharedSnapshotError: LocalizedError {
    case missingAppGroup

    var errorDescription: String? {
        "The shared TimeTracker app group is unavailable."
    }
}
