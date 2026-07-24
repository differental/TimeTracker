import ActivityKit
import AppIntents
import SwiftUI
import WidgetKit

struct TrackerLiveActivity: Widget {
    var body: some WidgetConfiguration {
        ActivityConfiguration(for: TrackerActivityAttributes.self) { context in
            lockScreenView(context)
        } dynamicIsland: { context in
            let state = TrackerState.by(id: context.state.stateID)
            return DynamicIsland {
                DynamicIslandExpandedRegion(.leading) {
                    HStack(spacing: 6) {
                        Image(systemName: state.symbolName)
                            .symbolRenderingMode(.hierarchical)
                            .foregroundStyle(state.color)
                        Text(state.name)
                    }
                    .font(.system(.subheadline, design: .rounded, weight: .semibold))
                    .lineLimit(1)
                    .minimumScaleFactor(0.65)
                    .frame(maxWidth: 132, alignment: .leading)
                    .padding(.leading, 4)
                }
                DynamicIslandExpandedRegion(.trailing) {
                    Text(context.state.start, style: .timer)
                        .font(.system(.headline, design: .rounded, weight: .bold))
                        .monospacedDigit()
                        .lineLimit(1)
                        .minimumScaleFactor(0.6)
                        .foregroundStyle(state.color)
                        .frame(width: 100, alignment: .trailing)
                        .padding(.trailing, 4)
                }
                DynamicIslandExpandedRegion(.bottom) {
                    VStack(spacing: 8) {
                        if context.isStale {
                            Label("Open TimeTracker to update", systemImage: "exclamationmark.arrow.trianglehead.2.clockwise.rotate.90")
                                .font(.caption)
                                .foregroundStyle(.orange)
                        } else {
                            Text("Tracking since \(context.state.start.formatted(date: .omitted, time: .shortened))")
                                .font(.caption)
                                .foregroundStyle(.secondary)
                        }
                        quickSwitches(excluding: state.id)
                    }
                }
            } compactLeading: {
                Image(systemName: state.symbolName)
                    .symbolRenderingMode(.hierarchical)
                    .foregroundStyle(state.color)
            } compactTrailing: {
                Text(context.state.start, style: .timer)
                    .monospacedDigit()
                    .foregroundStyle(state.color)
                    .frame(maxWidth: 44)
            } minimal: {
                Image(systemName: state.symbolName)
                    .symbolRenderingMode(.hierarchical)
                    .foregroundStyle(state.color)
            }
            .keylineTint(state.color)
            .widgetURL(URL(string: "timetracker://now"))
        }
    }

    private func lockScreenView(
        _ context: ActivityViewContext<TrackerActivityAttributes>
    ) -> some View {
        let state = context.state
        let tracker = TrackerState.by(id: state.stateID)
        return VStack(spacing: 10) {
            HStack(spacing: 12) {
                ZStack {
                    Circle()
                        .fill(tracker.color.opacity(0.25))
                        .frame(width: 44, height: 44)
                    Image(systemName: tracker.symbolName)
                        .font(.headline)
                        .symbolRenderingMode(.hierarchical)
                        .foregroundStyle(tracker.color)
                }
                VStack(alignment: .leading, spacing: 2) {
                    Text(tracker.name)
                        .font(.system(.headline, design: .rounded))
                    Text(
                        context.isStale
                            ? "Update needed"
                            : "since \(state.start.formatted(date: .omitted, time: .shortened))"
                    )
                    .font(.caption)
                    .foregroundStyle(context.isStale ? .orange : .secondary)
                }
                Spacer()
                Text(state.start, style: .timer)
                    .font(.system(.headline, design: .rounded, weight: .bold))
                    .monospacedDigit()
                    .lineLimit(1)
                    .minimumScaleFactor(0.6)
                    .foregroundStyle(tracker.color)
                    .frame(width: 100, alignment: .trailing)
            }
            quickSwitches(excluding: tracker.id)
        }
        .padding(16)
        .activityBackgroundTint(tracker.color.opacity(0.12))
        .activitySystemActionForegroundColor(.primary)
        .widgetURL(URL(string: "timetracker://now"))
    }

    private func quickSwitches(excluding currentID: Int) -> some View {
        let ids = (SharedSnapshotStore.load().predictedStateIDs + TrackerState.all.map(\.id))
            .filter { $0 != currentID && TrackerState.all.indices.contains($0) }
        let unique = ids.reduce(into: [Int]()) { result, id in
            if !result.contains(id), result.count < 2 {
                result.append(id)
            }
        }
        return HStack(spacing: 8) {
            ForEach(unique, id: \.self) { id in
                let state = TrackerState.by(id: id)
                Button(intent: SwitchActivityIntent(state: state)) {
                    Label(state.name, systemImage: state.symbolName)
                        .font(.caption.weight(.semibold))
                        .lineLimit(1)
                }
                .buttonStyle(.bordered)
                .tint(state.color)
            }
        }
    }
}
