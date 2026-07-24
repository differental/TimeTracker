import ActivityKit
import AlarmKit
import AppIntents
import SwiftUI
import WidgetKit

struct FocusAlarmLiveActivity: Widget {
    var body: some WidgetConfiguration {
        ActivityConfiguration(for: AlarmAttributes<FocusSessionMetadata>.self) { context in
            lockScreen(context)
        } dynamicIsland: { context in
            let state = context.attributes.metadata?.state ?? TrackerState.by(id: 1)
            return DynamicIsland {
                DynamicIslandExpandedRegion(.leading) {
                    Label(state.name, systemImage: state.symbolName)
                        .font(.headline)
                        .foregroundStyle(state.color)
                }
                DynamicIslandExpandedRegion(.trailing) {
                    remainingTime(context)
                        .font(.headline.monospacedDigit())
                }
                DynamicIslandExpandedRegion(.bottom) {
                    controls(context)
                }
            } compactLeading: {
                Image(systemName: state.symbolName)
                    .foregroundStyle(state.color)
            } compactTrailing: {
                remainingTime(context)
                    .font(.caption.monospacedDigit())
            } minimal: {
                Image(systemName: state.symbolName)
                    .foregroundStyle(state.color)
            }
            .keylineTint(state.color)
            .widgetURL(URL(string: "timetracker://now"))
        }
    }

    private func lockScreen(
        _ context: ActivityViewContext<AlarmAttributes<FocusSessionMetadata>>
    ) -> some View {
        let state = context.attributes.metadata?.state ?? TrackerState.by(id: 1)
        return VStack(spacing: 12) {
            HStack {
                Label(state.name, systemImage: state.symbolName)
                    .font(.headline)
                    .foregroundStyle(state.color)
                Spacer()
                remainingTime(context)
                    .font(.title3.monospacedDigit().bold())
            }
            controls(context)
        }
        .padding()
        .activityBackgroundTint(state.color.opacity(0.12))
        .activitySystemActionForegroundColor(.primary)
        .widgetURL(URL(string: "timetracker://now"))
    }

    @ViewBuilder
    private func remainingTime(
        _ context: ActivityViewContext<AlarmAttributes<FocusSessionMetadata>>
    ) -> some View {
        switch context.state.mode {
        case .countdown(let countdown):
            Text(timerInterval: countdown.startDate...countdown.fireDate, countsDown: true)
        case .paused(let paused):
            Text(formatDuration(paused.totalCountdownDuration - paused.previouslyElapsedDuration))
        case .alert:
            Text("Done")
        @unknown default:
            Text("Focus")
        }
    }

    @ViewBuilder
    private func controls(
        _ context: ActivityViewContext<AlarmAttributes<FocusSessionMetadata>>
    ) -> some View {
        HStack(spacing: 10) {
            switch context.state.mode {
            case .countdown:
                Button(intent: PauseFocusAlarmIntent(alarmID: context.state.alarmID)) {
                    Label("Pause", systemImage: "pause.fill")
                }
            case .paused:
                Button(intent: ResumeFocusAlarmIntent(alarmID: context.state.alarmID)) {
                    Label("Resume", systemImage: "play.fill")
                }
            case .alert:
                EmptyView()
            @unknown default:
                EmptyView()
            }
            Button(intent: CancelFocusAlarmIntent(alarmID: context.state.alarmID)) {
                Label("End", systemImage: "stop.fill")
            }
        }
        .buttonStyle(.bordered)
    }
}

struct PauseFocusAlarmIntent: LiveActivityIntent {
    static let title: LocalizedStringResource = "Pause Focus Timer"
    @Parameter var alarmID: String

    init() { alarmID = "" }
    init(alarmID: UUID) { self.alarmID = alarmID.uuidString }

    func perform() async throws -> some IntentResult {
        guard let id = UUID(uuidString: alarmID) else { return .result() }
        try AlarmManager.shared.pause(id: id)
        return .result()
    }
}

struct ResumeFocusAlarmIntent: LiveActivityIntent {
    static let title: LocalizedStringResource = "Resume Focus Timer"
    @Parameter var alarmID: String

    init() { alarmID = "" }
    init(alarmID: UUID) { self.alarmID = alarmID.uuidString }

    func perform() async throws -> some IntentResult {
        guard let id = UUID(uuidString: alarmID) else { return .result() }
        try AlarmManager.shared.resume(id: id)
        return .result()
    }
}

struct CancelFocusAlarmIntent: LiveActivityIntent {
    static let title: LocalizedStringResource = "End Focus Timer"
    @Parameter var alarmID: String

    init() { alarmID = "" }
    init(alarmID: UUID) { self.alarmID = alarmID.uuidString }

    func perform() async throws -> some IntentResult {
        guard let id = UUID(uuidString: alarmID) else { return .result() }
        try AlarmManager.shared.cancel(id: id)
        return .result()
    }
}
