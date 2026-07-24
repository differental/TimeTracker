import AppIntents
import Foundation
import SwiftUI
import WidgetKit

@MainActor
@Observable
final class AppModel {
    private static let defaultHistoryLengths = [1, 2, 4, 8, 16, 32]

    var serverURLString: String =
        ServerConfig.sharedDefaults?.string(forKey: "serverURL") ?? ""
    var accessKey: String = Keychain.read("accessKey") ?? ""
    var calendarSuggestionsEnabled: Bool =
        ServerConfig.sharedDefaults?.bool(forKey: "calendarSuggestionsEnabled") ?? false
    var predictionHistoryTableCount: Int = storedInteger(
        forKey: "predictionHistoryTableCount",
        default: 6,
        range: 1...6
    )
    var predictionBaseTableSize: Int = storedInteger(
        forKey: "predictionBaseTableSize",
        default: 512,
        range: 128...2_048
    )
    var predictionTaggedTableSize: Int = storedInteger(
        forKey: "predictionTaggedTableSize",
        default: 512,
        range: 128...2_048
    )
    var predictionUsefulnessAgingInterval: Int = storedInteger(
        forKey: "predictionUsefulnessAgingInterval",
        default: 256,
        range: 64...1_024
    )
    var predictionReplayWindowSize: Int = storedInteger(
        forKey: "predictionReplayWindowSize",
        default: 250,
        range: 25...1_000
    )

    init() {
        #if DEBUG
        let environment = ProcessInfo.processInfo.environment
        if let url = environment["TT_SERVER_URL"], let key = environment["TT_ACCESS_KEY"] {
            serverURLString = url
            accessKey = key
            saveSettings()
            UserDefaults.standard.set(0, forKey: "selectedTab")
        }
        #endif
    }

    var isConfigured: Bool { ServerConfig.load() != nil }

    func saveSettings() {
        let url = serverURLString.trimmingCharacters(in: .whitespacesAndNewlines)
        ServerConfig.sharedDefaults?.set(url, forKey: "serverURL")
        ServerConfig.sharedDefaults?.set(
            predictionHistoryTableCount,
            forKey: "predictionHistoryTableCount"
        )
        ServerConfig.sharedDefaults?.set(
            predictionBaseTableSize,
            forKey: "predictionBaseTableSize"
        )
        ServerConfig.sharedDefaults?.set(
            predictionTaggedTableSize,
            forKey: "predictionTaggedTableSize"
        )
        ServerConfig.sharedDefaults?.set(
            predictionUsefulnessAgingInterval,
            forKey: "predictionUsefulnessAgingInterval"
        )
        ServerConfig.sharedDefaults?.set(
            predictionReplayWindowSize,
            forKey: "predictionReplayWindowSize"
        )
        do {
            try Keychain.save(
                accessKey.trimmingCharacters(in: .whitespacesAndNewlines),
                for: "accessKey"
            )
            lastError = nil
            WidgetCenter.shared.reloadAllTimelines()
        } catch {
            lastError = error.localizedDescription
        }
    }

    var client: APIClient? {
        ServerConfig.load().map(APIClient.init)
    }

    private(set) var recents: [EntryRecord] = []
    private(set) var summary: [StateShare] = []
    private(set) var todayTotals: [Int64] =
        Array(repeating: 0, count: TrackerState.all.count)
    private(set) var predictedStates: [TrackerState] = Array(TrackerState.all.prefix(3))
    var summaryDays = 7
    private var summaryRequest = 0
    private(set) var analyticsEntries: [EntryRecord] = []
    private var activityPredictor = ActivityPredictor(entries: [])

    private(set) var lastError: String?

    var current: EntryRecord? { recents.first }

    func refresh() async {
        guard let client else {
            lastError = APIError.notConfigured.localizedDescription
            await LiveActivityController.endAll()
            return
        }
        let previousCurrent = current.map { ($0.stateID, $0.startTimestamp) }
        do {
            async let summaryTask = client.summary(days: summaryDays)
            async let todayTask = client.summary(days: 1)
            let (length, rawRecents) = try await stableRecents(using: client)
            let (rawSummary, rawToday) = try await (summaryTask, todayTask)

            let entries = makeEntries(length: length, rawEntries: rawRecents)
            analyticsEntries = entries
            recents = Array(entries.prefix(300))
            activityPredictor = ActivityPredictor(
                entries: analyticsEntries,
                configuration: predictionConfiguration
            )
            recomputePredictions()
            await applyCalendarSuggestion()
            let refreshedCurrent = current.map { ($0.stateID, $0.startTimestamp) }
            if previousCurrent?.0 != refreshedCurrent?.0
                || previousCurrent?.1 != refreshedCurrent?.1 {
                WidgetCenter.shared.reloadAllTimelines()
            }
            summary = zip(TrackerState.all, rawSummary)
                .map { StateShare(state: $0, milliseconds: $1) }
                .sorted { $0.milliseconds > $1.milliseconds }
            todayTotals = rawToday
            lastError = nil
            persistSharedSnapshot()
            await LiveActivityController.reconcile(
                stateID: current?.stateID,
                startTimestamp: current?.startTimestamp,
                createIfNeeded: false
            )
        } catch {
            lastError = error.localizedDescription
        }
    }

    func setSummaryDays(_ days: Int) async {
        summaryDays = days
        summaryRequest += 1
        let request = summaryRequest
        guard let client else { return }
        do {
            let raw = try await client.summary(days: days)
            guard request == summaryRequest else { return }
            summary = zip(TrackerState.all, raw)
                .map { StateShare(state: $0, milliseconds: $1) }
                .sorted { $0.milliseconds > $1.milliseconds }
        } catch {
            lastError = error.localizedDescription
        }
    }

    func switchState(to state: TrackerState, at date: Date) async throws {
        guard let client else { throw APIError.notConfigured }
        FocusSessionScheduler.cancelActive()
        await LiveActivityController.endAll()
        let timestamp = Int64(date.timeIntervalSince1970 * 1000)
        _ = try await client.addEntry(
            stateID: state.id,
            startTimestamp: timestamp,
            force: true
        )
        _ = try? await SwitchActivityIntent(state: state).donate()
        WidgetCenter.shared.reloadAllTimelines()
        await refresh()
    }

    func recomputePredictions(at date: Date = .now) {
        predictedStates = activityPredictor.predictions(
            at: date,
            currentStateID: current?.stateID
        )
    }

    func testPredictionAccuracy() async throws -> ActivityPredictor.BacktestResult {
        guard let client else { throw APIError.notConfigured }
        let (length, rawEntries) = try await stableRecents(
            using: client,
            count: nil
        )
        let entries = makeEntries(length: length, rawEntries: rawEntries)
        let configuration = predictionConfiguration
        let windowSize = predictionReplayWindowSize
        return try await Task.detached(priority: .userInitiated) {
            try ActivityPredictor.backtest(
                entries: entries,
                configuration: configuration,
                windowSize: windowSize
            )
        }.value
    }

    func setCalendarSuggestionsEnabled(_ enabled: Bool) async {
        if enabled {
            do {
                guard try await CalendarContextProvider().requestAccess() else {
                    calendarSuggestionsEnabled = false
                    lastError = "Calendar access wasn’t granted."
                    return
                }
            } catch {
                calendarSuggestionsEnabled = false
                lastError = error.localizedDescription
                return
            }
        }
        calendarSuggestionsEnabled = enabled
        ServerConfig.sharedDefaults?.set(
            enabled,
            forKey: "calendarSuggestionsEnabled"
        )
        await applyCalendarSuggestion()
        persistSharedSnapshot()
    }

    func startFocusSession(
        activity: TrackerState,
        durationMinutes: Int
    ) async throws {
        try await FocusSessionScheduler.start(
            activity: TrackerActivityEntity(activity),
            durationMinutes: durationMinutes
        )
        _ = try? await StartFocusSessionIntent(
            activity: TrackerActivityEntity(activity),
            minutes: durationMinutes
        ).donate()
        await refresh()
    }

    func updateEntry(
        _ entry: EntryRecord,
        stateID: Int?,
        start: Date?
    ) async throws {
        guard let client else { throw APIError.notConfigured }
        let timestamp = start.map { Int64($0.timeIntervalSince1970 * 1000) }
        _ = try await client.updateEntry(
            index: entry.index,
            stateID: stateID,
            startTimestamp: timestamp,
            force: false
        )
        WidgetCenter.shared.reloadAllTimelines()
        await refresh()
    }

    private func stableRecents(
        using client: APIClient,
        count requestedCount: Int? = 10_000
    ) async throws -> (length: UInt64, entries: [[Int64]]) {
        for _ in 0..<3 {
            let before = try await client.length()
            let count = requestedCount
                ?? Int(min(before, UInt64(Int.max)))
            let entries = try await client.recents(
                count: count,
                days: Int(UInt32.max)
            )
            let after = try await client.length()
            if before == after {
                return (after, entries)
            }
        }
        throw APIError.changedDuringRefresh
    }

    private var predictionConfiguration: ActivityPredictor.Configuration {
        ActivityPredictor.Configuration(
            historyLengths: Array(
                Self.defaultHistoryLengths.prefix(predictionHistoryTableCount)
            ),
            baseTableSize: predictionBaseTableSize,
            taggedTableSize: predictionTaggedTableSize,
            usefulnessAgingInterval: predictionUsefulnessAgingInterval,
            maximumTrainingEntries: 10_000
        )
    }

    private func makeEntries(
        length: UInt64,
        rawEntries: [[Int64]]
    ) -> [EntryRecord] {
        var entries: [EntryRecord] = []
        for (offset, pair) in rawEntries.enumerated() where pair.count == 2 {
            entries.append(
                EntryRecord(
                    index: length - 1 - UInt64(offset),
                    stateID: Int(pair[0]),
                    startTimestamp: pair[1],
                    endTimestamp: entries.last?.startTimestamp
                )
            )
        }
        return entries
    }

    var totalTracked: TimeInterval {
        summary.reduce(0) { $0 + $1.duration }
    }

    private func persistSharedSnapshot() {
        let snapshot = SharedTrackerSnapshot(
            updatedAt: .now,
            currentStateID: current?.stateID,
            currentStartTimestamp: current?.startTimestamp,
            todayTotals: todayTotals,
            predictedStateIDs: predictedStates.map(\.id)
        )
        do {
            try SharedSnapshotStore.save(snapshot)
        } catch {
            lastError = error.localizedDescription
        }
    }

    private func applyCalendarSuggestion() async {
        guard calendarSuggestionsEnabled,
              let calendarState = await CalendarContextProvider().suggestedActivity(),
              calendarState.id != current?.stateID
        else { return }
        predictedStates.removeAll { $0.id == calendarState.id }
        predictedStates.insert(calendarState, at: 0)
        predictedStates = Array(predictedStates.prefix(3))
    }
}

private func storedInteger(
    forKey key: String,
    default defaultValue: Int,
    range: ClosedRange<Int>
) -> Int {
    guard let value = ServerConfig.sharedDefaults?.object(forKey: key) as? Int else {
        return defaultValue
    }
    return min(max(value, range.lowerBound), range.upperBound)
}
