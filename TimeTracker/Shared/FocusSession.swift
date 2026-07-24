import AlarmKit
import AppIntents
import Foundation
import SwiftUI
import WidgetKit

struct FocusSessionMetadata: AlarmMetadata {
    let stateID: Int
    let startTimestamp: Int64
    let endTimestamp: Int64

    var state: TrackerState {
        TrackerState.by(id: stateID)
    }
}

@MainActor
enum FocusSessionScheduler {
    private static let activeAlarmKey = "activeFocusAlarmID"

    static func start(
        activity: TrackerActivityEntity,
        durationMinutes: Int
    ) async throws {
        guard (1...480).contains(durationMinutes) else {
            throw FocusSessionError.invalidDuration
        }
        guard let config = ServerConfig.load() else {
            throw TrackerIntentError.notConfigured
        }

        let authorization: AlarmManager.AuthorizationState
        switch AlarmManager.shared.authorizationState {
        case .notDetermined:
            authorization = try await AlarmManager.shared.requestAuthorization()
        case let state:
            authorization = state
        }
        guard authorization == .authorized else {
            throw FocusSessionError.alarmPermissionDenied
        }

        cancelActive()
        await LiveActivityController.endAll()

        let now = Int64(Date.now.timeIntervalSince1970 * 1_000)
        let trackingStartTimestamp: Int64
        do {
            let response = try await APIClient(config: config).addEntry(
                stateID: activity.id,
                startTimestamp: now,
                force: false
            )
            trackingStartTimestamp = response.start_timestamp
        } catch APIError.server(_, let message) where message.contains("same as current") {
            trackingStartTimestamp =
                SharedSnapshotStore.load().currentStartTimestamp ?? now
        }
        let duration = TimeInterval(durationMinutes * 60)
        let endTimestamp = now + Int64(duration * 1_000)
        let state = activity.state
        let title = LocalizedStringResource(
            stringLiteral: "\(state.name) session finished"
        )
        let countdownTitle = LocalizedStringResource(
            stringLiteral: "\(state.name) focus"
        )
        let presentation = AlarmPresentation(
            alert: AlarmPresentation.Alert(title: title),
            countdown: AlarmPresentation.Countdown(
                title: countdownTitle,
                pauseButton: AlarmButton(
                    text: "Pause",
                    textColor: state.color,
                    systemImageName: "pause.fill"
                )
            ),
            paused: AlarmPresentation.Paused(
                title: "Focus paused",
                resumeButton: AlarmButton(
                    text: "Resume",
                    textColor: state.color,
                    systemImageName: "play.fill"
                )
            )
        )
        let metadata = FocusSessionMetadata(
            stateID: state.id,
            startTimestamp: now,
            endTimestamp: endTimestamp
        )
        let attributes = AlarmAttributes(
            presentation: presentation,
            metadata: metadata,
            tintColor: state.color
        )
        let alarmID = UUID()
        let configuration = AlarmManager.AlarmConfiguration(
            countdownDuration: Alarm.CountdownDuration(
                preAlert: duration,
                postAlert: nil
            ),
            attributes: attributes
        )
        _ = try await AlarmManager.shared.schedule(
            id: alarmID,
            configuration: configuration
        )
        ServerConfig.sharedDefaults?.set(
            alarmID.uuidString,
            forKey: activeAlarmKey
        )

        var snapshot = SharedSnapshotStore.load()
        snapshot.updatedAt = .now
        snapshot.currentStateID = state.id
        snapshot.currentStartTimestamp = trackingStartTimestamp
        try? SharedSnapshotStore.save(snapshot)
        WidgetCenter.shared.reloadAllTimelines()
    }

    static func cancelActive() {
        guard let raw = ServerConfig.sharedDefaults?.string(forKey: activeAlarmKey),
              let id = UUID(uuidString: raw)
        else { return }
        try? AlarmManager.shared.cancel(id: id)
        ServerConfig.sharedDefaults?.removeObject(forKey: activeAlarmKey)
    }
}

struct StartFocusSessionIntent: AppIntent, LiveActivityIntent {
    static let title: LocalizedStringResource = "Start Focus Session"
    static let description = IntentDescription(
        "Start tracking an activity with a bounded system timer."
    )

    @Parameter(title: "Activity")
    var activity: TrackerActivityEntity

    @Parameter(
        title: "Minutes",
        controlStyle: .field,
        inclusiveRange: (1, 480)
    )
    var minutes: Int

    init() {
        activity = TrackerActivityEntity(id: 1)
        minutes = 45
    }

    init(activity: TrackerActivityEntity, minutes: Int) {
        self.activity = activity
        self.minutes = minutes
    }

    static var parameterSummary: some ParameterSummary {
        Summary("Track \(\.$activity) for \(\.$minutes) minutes")
    }

    func perform() async throws -> some IntentResult & ProvidesDialog {
        try await FocusSessionScheduler.start(
            activity: activity,
            durationMinutes: minutes
        )
        return .result(
            dialog: "Started a \(minutes)-minute \(activity.state.name) session."
        )
    }
}

enum FocusSessionError: Error, LocalizedError {
    case invalidDuration
    case alarmPermissionDenied

    var errorDescription: String? {
        switch self {
        case .invalidDuration:
            "Choose a focus duration between 1 minute and 8 hours."
        case .alarmPermissionDenied:
            "Allow TimeTracker alarms in Settings to start focus sessions."
        }
    }
}
