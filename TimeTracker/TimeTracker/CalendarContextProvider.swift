import EventKit
import Foundation

@MainActor
struct CalendarContextProvider {
    private let store = EKEventStore()

    func requestAccess() async throws -> Bool {
        switch EKEventStore.authorizationStatus(for: .event) {
        case .fullAccess:
            true
        case .notDetermined, .writeOnly:
            try await store.requestFullAccessToEvents()
        default:
            false
        }
    }

    func suggestedActivity(at date: Date = .now) async -> TrackerState? {
        guard EKEventStore.authorizationStatus(for: .event) == .fullAccess else {
            return nil
        }
        let end = date.addingTimeInterval(60 * 60)
        let predicate = store.predicateForEvents(
            withStart: date.addingTimeInterval(-15 * 60),
            end: end,
            calendars: nil
        )
        guard let event = store.events(matching: predicate)
            .filter({ !$0.isAllDay && $0.endDate > date })
            .sorted(by: { $0.startDate < $1.startDate })
            .first
        else {
            return nil
        }

        // Event titles never leave the device. They are used only to choose one
        // of TimeTracker's existing categories.
        let text = "\(event.title ?? "") \(event.notes ?? "")".lowercased()
        let mappings: [(keywords: [String], stateID: Int)] = [
            (["work", "standup", "office", "client"], 1),
            (["study", "lecture", "class", "exam"], 0),
            (["sport", "gym", "run", "training"], 11),
            (["commute", "train", "travel"], 2),
            (["social", "dinner", "friends"], 10),
        ]
        return mappings.first(where: { mapping in
            mapping.keywords.contains(where: text.contains)
        }).map { TrackerState.by(id: $0.stateID) }
            ?? TrackerState.by(id: 9)
    }
}
