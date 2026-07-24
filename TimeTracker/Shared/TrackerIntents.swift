import AppIntents
import CoreSpotlight
import Foundation
import SwiftUI
import WidgetKit

struct TrackerActivityEntity: AppEntity, IndexedEntity, Hashable, Sendable {
    static let typeDisplayRepresentation = TypeDisplayRepresentation(
        name: "Activity",
        numericFormat: "\(placeholder: .int) activities"
    )
    static let defaultQuery = TrackerActivityQuery()

    let id: Int

    init(id: Int) {
        self.id = id
    }

    init(_ state: TrackerState) {
        id = state.id
    }

    var state: TrackerState {
        TrackerState.by(id: id)
    }

    var displayRepresentation: DisplayRepresentation {
        DisplayRepresentation(
            title: LocalizedStringResource(stringLiteral: state.name),
            subtitle: "TimeTracker activity",
            image: .init(systemName: state.symbolName)
        )
    }
}

struct TrackerActivityQuery: EntityStringQuery, EnumerableEntityQuery {
    func entities(for identifiers: [Int]) async throws -> [TrackerActivityEntity] {
        identifiers.compactMap { identifier in
            guard TrackerState.all.indices.contains(identifier) else { return nil }
            return TrackerActivityEntity(id: identifier)
        }
    }

    func suggestedEntities() async throws -> [TrackerActivityEntity] {
        let snapshot = SharedSnapshotStore.load()
        let orderedIDs = snapshot.predictedStateIDs + TrackerState.all.map(\.id)
        var seen = Set<Int>()
        return orderedIDs.compactMap { id in
            guard TrackerState.all.indices.contains(id), seen.insert(id).inserted else {
                return nil
            }
            return TrackerActivityEntity(id: id)
        }
    }

    func allEntities() async throws -> [TrackerActivityEntity] {
        TrackerState.all.map(TrackerActivityEntity.init)
    }

    func entities(matching string: String) async throws -> [TrackerActivityEntity] {
        let query = string.trimmingCharacters(in: .whitespacesAndNewlines)
        guard !query.isEmpty else { return try await suggestedEntities() }
        return TrackerState.all
            .filter { $0.name.localizedCaseInsensitiveContains(query) }
            .map(TrackerActivityEntity.init)
    }
}

struct SwitchActivityIntent: AppIntent, LiveActivityIntent {
    static let title: LocalizedStringResource = "Switch Activity"
    static let description = IntentDescription(
        "Start tracking a different activity in TimeTracker."
    )

    @Parameter(title: "Activity")
    var activity: TrackerActivityEntity

    init() {
        activity = TrackerActivityEntity(id: 1)
    }

    init(activity: TrackerActivityEntity) {
        self.activity = activity
    }

    init(state: TrackerState) {
        activity = TrackerActivityEntity(state)
    }

    static var parameterSummary: some ParameterSummary {
        Summary("Start tracking \(\.$activity)")
    }

    @MainActor
    func perform() async throws -> some IntentResult & ProvidesDialog {
        guard let config = ServerConfig.load() else {
            throw TrackerIntentError.notConfigured
        }
        let client = APIClient(config: config)
        let state = activity.state
        let now = Int64(Date().timeIntervalSince1970 * 1_000)
        FocusSessionScheduler.cancelActive()
        await LiveActivityController.endAll()
        do {
            let response = try await client.addEntry(
                stateID: state.id,
                startTimestamp: now,
                force: false
            )
            updateSharedSnapshot(
                stateID: response.new_state,
                startTimestamp: response.start_timestamp
            )
        } catch APIError.server(_, let message) where message.contains("same as current") {
            return .result(dialog: "You're already tracking \(state.name).")
        }
        WidgetCenter.shared.reloadAllTimelines()
        CurrentActivitySnippetIntent.reload()
        return .result(dialog: "Now tracking \(state.name).")
    }
}

struct CurrentActivityIntent: AppIntent {
    static let title: LocalizedStringResource = "Current Activity"
    static let description = IntentDescription(
        "Check what TimeTracker is currently tracking and switch activities."
    )

    @MainActor
    func perform() async throws -> some IntentResult & ProvidesDialog & ShowsSnippetIntent {
        let current = try await fetchCurrentActivity()
        let dialog: IntentDialog
        if let current {
            let elapsed = formatDuration(
                Date().timeIntervalSince(current.start),
                compact: true
            )
            dialog = "\(current.state.name), for \(elapsed) so far."
        } else {
            dialog = "Nothing is being tracked right now."
        }
        return .result(
            dialog: dialog,
            snippetIntent: CurrentActivitySnippetIntent()
        )
    }
}

struct CurrentActivitySnippetIntent: SnippetIntent {
    static let title: LocalizedStringResource = "Current Activity Controls"
    static let isDiscoverable = false

    @MainActor
    func perform() async throws -> some IntentResult & ShowsSnippetView {
        let current = try await fetchCurrentActivity()
        let snapshot = SharedSnapshotStore.load()
        let currentID = current?.state.id
        let suggested = (snapshot.predictedStateIDs + TrackerState.all.map(\.id))
            .filter { $0 != currentID && TrackerState.all.indices.contains($0) }
            .reduce(into: [Int]()) { result, id in
                if !result.contains(id), result.count < 3 {
                    result.append(id)
                }
            }
            .map { TrackerActivityEntity(id: $0) }
        return .result(
            view: CurrentActivitySnippetView(
                current: current,
                suggestions: suggested
            )
        )
    }
}

private struct CurrentActivitySnippetView: View {
    let current: CurrentTrackedActivity?
    let suggestions: [TrackerActivityEntity]

    var body: some View {
        VStack(alignment: .leading, spacing: 12) {
            if let current {
                HStack(spacing: 10) {
                    Image(systemName: current.state.symbolName)
                        .font(.title2)
                        .foregroundStyle(current.state.color)
                    VStack(alignment: .leading, spacing: 2) {
                        Text(current.state.name)
                            .font(.headline)
                        Text(current.start, style: .timer)
                            .font(.title3.monospacedDigit().bold())
                    }
                }
            } else {
                Label("Nothing tracked yet", systemImage: "timer")
                    .font(.headline)
            }

            HStack(spacing: 8) {
                ForEach(suggestions) { activity in
                    Button(intent: SwitchActivityIntent(activity: activity)) {
                        Label(
                            activity.state.name,
                            systemImage: activity.state.symbolName
                        )
                        .lineLimit(1)
                    }
                    .buttonStyle(.bordered)
                }
            }
        }
        .padding()
    }
}

struct OpenActivityIntent: OpenIntent {
    static let title: LocalizedStringResource = "Open Activity"

    @Parameter(title: "Activity")
    var target: TrackerActivityEntity

    init() {
        target = TrackerActivityEntity(id: 1)
    }

    init(target: TrackerActivityEntity) {
        self.target = target
    }
}

struct TrackingFocusFilterIntent: SetFocusFilterIntent {
    static let title: LocalizedStringResource = "Tracking Activity"

    @Parameter(title: "Activity")
    var activity: TrackerActivityEntity?

    init() {
        activity = TrackerActivityEntity(id: 1)
    }

    init(activity: TrackerActivityEntity) {
        self.activity = activity
    }

    var displayRepresentation: DisplayRepresentation {
        let state = activity?.state ?? TrackerState.by(id: 1)
        return DisplayRepresentation(
            title: "Track \(state.name)",
            image: .init(systemName: state.symbolName)
        )
    }

    static func suggestedFocusFilters(
        for context: FocusFilterSuggestionContext
    ) async -> [TrackingFocusFilterIntent] {
        [1, 0, 7].map {
            TrackingFocusFilterIntent(
                activity: TrackerActivityEntity(id: $0)
            )
        }
    }

    @MainActor
    func perform() async throws -> some IntentResult {
        guard let activity else { return .result() }
        guard let config = ServerConfig.load() else {
            throw TrackerIntentError.notConfigured
        }
        FocusSessionScheduler.cancelActive()
        await LiveActivityController.endAll()
        let now = Int64(Date.now.timeIntervalSince1970 * 1_000)
        do {
            let response = try await APIClient(config: config).addEntry(
                stateID: activity.id,
                startTimestamp: now,
                force: false
            )
            updateSharedSnapshot(
                stateID: response.new_state,
                startTimestamp: response.start_timestamp
            )
            WidgetCenter.shared.reloadAllTimelines()
        } catch APIError.server(_, let message) where message.contains("same as current") {
            // A Focus reactivation is idempotent when this activity is current.
        }
        return .result()
    }
}

struct CurrentTrackedActivity: Sendable {
    let state: TrackerState
    let start: Date
}

@MainActor
private func fetchCurrentActivity() async throws -> CurrentTrackedActivity? {
    guard let config = ServerConfig.load() else {
        throw TrackerIntentError.notConfigured
    }
    let client = APIClient(config: config)
    let recents = try await client.recents(
        count: 1,
        days: Int(UInt32.max)
    )
    guard let pair = recents.first, pair.count == 2 else {
        return nil
    }
    return CurrentTrackedActivity(
        state: TrackerState.by(id: Int(pair[0])),
        start: Date(timeIntervalSince1970: Double(pair[1]) / 1_000)
    )
}

private func updateSharedSnapshot(stateID: Int, startTimestamp: Int64) {
    var snapshot = SharedSnapshotStore.load()
    snapshot.updatedAt = .now
    snapshot.currentStateID = stateID
    snapshot.currentStartTimestamp = startTimestamp
    try? SharedSnapshotStore.save(snapshot)
}

enum TrackerIntentError: Error, CustomLocalizedStringResourceConvertible {
    case notConfigured

    var localizedStringResource: LocalizedStringResource {
        "TimeTracker isn't connected to a server yet. Open the app and configure it in Settings."
    }
}
