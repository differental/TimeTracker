import SwiftUI

nonisolated struct TrackerState: Identifiable, Hashable, Sendable {
    let id: Int
    let symbolName: String
    let name: String
    let color: Color

    static let all: [TrackerState] = [
        .init(id: 0, symbolName: "book.closed.fill", name: "Study",
              color: Color(hex: 0x4a71ea)),
        .init(id: 1, symbolName: "briefcase.fill", name: "Work",
              color: Color(hex: 0xd4b37f)),
        .init(id: 2, symbolName: "tram.fill", name: "Commute",
              color: Color(hex: 0xff8c00)),
        .init(id: 3, symbolName: "laptopcomputer", name: "Projects",
              color: Color(hex: 0xc49aff)),
        .init(id: 4, symbolName: "play.rectangle.fill", name: "Entertainment",
              color: Color(hex: 0xffe066)),
        .init(id: 5, symbolName: "lightbulb.fill", name: "Exploration",
              color: Color(hex: 0x2ecc71)),
        .init(id: 6, symbolName: "fork.knife", name: "Maintenance",
              color: Color(hex: 0xb56a3b)),
        .init(id: 7, symbolName: "bed.double.fill", name: "Sleep",
              color: Color(hex: 0xffd6e8)),
        .init(id: 8, symbolName: "checkmark.seal.fill", name: "Mission",
              color: Color(hex: 0x008080)),
        .init(id: 9, symbolName: "calendar", name: "Appointment",
              color: Color(hex: 0x6f42c1)),
        .init(id: 10, symbolName: "bubble.left.and.bubble.right.fill", name: "Social",
              color: Color(hex: 0xff6b6b)),
        .init(id: 11, symbolName: "figure.run", name: "Sports",
              color: Color(hex: 0xe74c3c)),
        .init(id: 12, symbolName: "airplane", name: "Holiday",
              color: Color(hex: 0xfff9ba)),
        .init(id: 13, symbolName: "ellipsis", name: "Other",
              color: Color(hex: 0x5b5b60)),
        .init(id: 14, symbolName: "exclamationmark.triangle.fill", name: "Emergency",
              color: Color(hex: 0xff2d2d)),
    ]

    static func by(id: Int) -> TrackerState {
        precondition(all.indices.contains(id), "Unexpected tracker state ID: \(id)")
        return all[id]
    }
}

nonisolated struct EntryRecord: Identifiable, Hashable, Sendable {
    let index: UInt64
    let stateID: Int
    let startTimestamp: Int64
    var endTimestamp: Int64?

    var id: UInt64 { index }
    var state: TrackerState { .by(id: stateID) }
    var startDate: Date { Date(timeIntervalSince1970: Double(startTimestamp) / 1000) }
    var endDate: Date? { endTimestamp.map { Date(timeIntervalSince1970: Double($0) / 1000) } }
    var duration: TimeInterval? { endTimestamp.map { Double($0 - startTimestamp) / 1000 } }
}

nonisolated struct StateShare: Identifiable, Sendable {
    let state: TrackerState
    let milliseconds: Int64
    var id: Int { state.id }
    var duration: TimeInterval { Double(milliseconds) / 1000 }
}

extension Color {
    nonisolated init(hex: UInt32) {
        self.init(
            red: Double((hex >> 16) & 0xFF) / 255,
            green: Double((hex >> 8) & 0xFF) / 255,
            blue: Double(hex & 0xFF) / 255
        )
    }
}

nonisolated func formatDuration(_ interval: TimeInterval, compact: Bool = false) -> String {
    let total = max(0, Int(interval.rounded()))
    let hours = total / 3_600
    let minutes = (total % 3_600) / 60
    if compact {
        if hours > 0 { return "\(hours)h \(minutes)m" }
        if minutes > 0 { return "\(minutes)m" }
        return "\(total % 60)s"
    }
    return String(format: "%02d:%02d:%02d", hours, minutes, total % 60)
}
