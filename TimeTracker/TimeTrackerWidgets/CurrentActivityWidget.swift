import AppIntents
import SwiftUI
import WidgetKit

struct ActivityEntry: TimelineEntry {
    let date: Date
    let currentStateID: Int?
    let start: Date?
    let topToday: [(stateID: Int, milliseconds: Int64)]
    let predictedStateIDs: [Int]
    let lastUpdated: Date?
    let isConfigured: Bool
}

struct ActivityProvider: TimelineProvider {
    func placeholder(in context: Context) -> ActivityEntry {
        ActivityEntry(
            date: .now,
            currentStateID: 3,
            start: .now.addingTimeInterval(-3_600),
            topToday: [
                (1, Int64(5) * 3_600_000),
                (3, Int64(2) * 3_600_000),
                (6, Int64(3_600_000)),
            ],
            predictedStateIDs: [1, 0, 6],
            lastUpdated: .now,
            isConfigured: true
        )
    }

    func getSnapshot(
        in context: Context,
        completion: @escaping (ActivityEntry) -> Void
    ) {
        if context.isPreview {
            completion(placeholder(in: context))
        } else {
            completion(entry(from: SharedSnapshotStore.load()))
        }
    }

    func getTimeline(
        in context: Context,
        completion: @escaping (Timeline<ActivityEntry>) -> Void
    ) {
        Task {
            completion(
                Timeline(
                    entries: [await fetch()],
                    policy: .after(.now.addingTimeInterval(30 * 60))
                )
            )
        }
    }

    func relevance() async -> WidgetRelevance<Void> {
        WidgetRelevance([
            WidgetRelevanceAttribute(context: .location(inferred: .work)),
            WidgetRelevanceAttribute(context: .location(inferred: .school)),
            WidgetRelevanceAttribute(context: .location(inferred: .commute)),
            WidgetRelevanceAttribute(context: .sleep(.bedtime)),
            WidgetRelevanceAttribute(context: .sleep(.wakeup)),
        ])
    }

    private func fetch() async -> ActivityEntry {
        let cached = SharedSnapshotStore.load()
        guard let config = ServerConfig.load() else {
            return entry(from: cached, isConfigured: false)
        }
        let client = APIClient(config: config)
        do {
            async let recents = client.recents(
                count: 1,
                days: Int(UInt32.max)
            )
            async let summary = client.summary(days: 1)
            let (recentEntries, totals) = try await (recents, summary)
            let pair = recentEntries.first
            let snapshot = SharedTrackerSnapshot(
                updatedAt: .now,
                currentStateID: pair.map { Int($0[0]) },
                currentStartTimestamp: pair.map { $0[1] },
                todayTotals: totals,
                predictedStateIDs: cached.predictedStateIDs
            )
            try? SharedSnapshotStore.save(snapshot)
            return entry(from: snapshot)
        } catch {
            return entry(from: cached)
        }
    }

    private func entry(
        from snapshot: SharedTrackerSnapshot,
        isConfigured: Bool = ServerConfig.load() != nil
    ) -> ActivityEntry {
        let top = snapshot.todayTotals.enumerated()
            .filter { $0.element > 0 }
            .sorted { $0.element > $1.element }
            .prefix(3)
            .map { (stateID: $0.offset, milliseconds: $0.element) }
        return ActivityEntry(
            date: .now,
            currentStateID: snapshot.currentStateID,
            start: snapshot.currentStart,
            topToday: top,
            predictedStateIDs: snapshot.predictedStateIDs,
            lastUpdated: snapshot.isAvailable ? snapshot.updatedAt : nil,
            isConfigured: isConfigured
        )
    }
}

struct CurrentActivityWidgetView: View {
    var entry: ActivityEntry

    @Environment(\.widgetFamily) private var family
    @Environment(\.widgetRenderingMode) private var renderingMode

    var body: some View {
        Group {
            switch family {
            case .accessoryCircular:
                accessoryCircular
            case .accessoryRectangular:
                accessoryRectangular
            case .accessoryInline:
                accessoryInline
            case .systemMedium:
                mediumView
            default:
                smallView
            }
        }
        .privacySensitive()
        .widgetURL(URL(string: "timetracker://now"))
        .containerBackground(for: .widget) {
            if renderingMode == .fullColor {
                backgroundGradient
            } else {
                Color.clear
            }
        }
    }

    @ViewBuilder
    private var smallView: some View {
        if let state, let start = entry.start {
            VStack(alignment: .leading, spacing: 4) {
                Image(systemName: state.symbolName)
                    .font(.title2.weight(.semibold))
                    .symbolRenderingMode(.hierarchical)
                    .foregroundStyle(state.color)
                    .widgetAccentable()
                Spacer(minLength: 2)
                Text(state.name)
                    .font(.system(.subheadline, design: .rounded, weight: .semibold))
                    .lineLimit(1)
                    .minimumScaleFactor(0.7)
                Text(start, style: .timer)
                    .font(.system(.title3, design: .rounded, weight: .bold))
                    .monospacedDigit()
                    .foregroundStyle(state.color)
                    .widgetAccentable()
                updatedLabel
            }
            .frame(maxWidth: .infinity, maxHeight: .infinity, alignment: .leading)
        } else {
            emptyView
        }
    }

    @ViewBuilder
    private var mediumView: some View {
        if let state, let start = entry.start {
            HStack(spacing: 14) {
                VStack(alignment: .leading, spacing: 5) {
                    Label(state.name, systemImage: state.symbolName)
                        .font(.system(.headline, design: .rounded, weight: .semibold))
                        .foregroundStyle(state.color)
                        .widgetAccentable()
                        .lineLimit(1)
                    Text(start, style: .timer)
                        .font(.system(.title2, design: .rounded, weight: .bold))
                        .monospacedDigit()
                    updatedLabel
                    Spacer(minLength: 2)
                    quickSwitches(excluding: state.id)
                }
                Divider()
                todaySummary
            }
        } else {
            emptyView
        }
    }

    private var accessoryCircular: some View {
        ZStack {
            AccessoryWidgetBackground()
            if let state {
                VStack(spacing: 1) {
                    Image(systemName: state.symbolName)
                        .font(.headline)
                    if let start = entry.start {
                        Text(start, style: .timer)
                            .font(.caption2.monospacedDigit())
                            .minimumScaleFactor(0.55)
                    }
                }
                .widgetAccentable()
            } else {
                Image(systemName: "timer")
            }
        }
    }

    private var accessoryRectangular: some View {
        HStack(spacing: 8) {
            Image(systemName: state?.symbolName ?? "timer")
                .font(.title3)
                .widgetAccentable()
            VStack(alignment: .leading, spacing: 1) {
                Text(state?.name ?? "Nothing tracked")
                    .font(.headline)
                    .lineLimit(1)
                if let start = entry.start {
                    Text(start, style: .timer)
                        .font(.caption.monospacedDigit())
                } else {
                    Text(entry.isConfigured ? "Tap to start" : "Open to connect")
                        .font(.caption)
                }
            }
        }
    }

    @ViewBuilder
    private var accessoryInline: some View {
        if let state, let start = entry.start {
            Label {
                Text("\(state.name) · \(start, style: .timer)")
            } icon: {
                Image(systemName: state.symbolName)
            }
        } else {
            Label("Nothing tracked", systemImage: "timer")
        }
    }

    private var todaySummary: some View {
        VStack(alignment: .leading, spacing: 6) {
            Text("TODAY")
                .font(.caption2.weight(.semibold))
                .foregroundStyle(.secondary)
            if entry.topToday.isEmpty {
                Text("No completed time yet")
                    .font(.caption)
                    .foregroundStyle(.secondary)
            } else {
                ForEach(entry.topToday, id: \.stateID) { item in
                    let itemState = TrackerState.by(id: item.stateID)
                    HStack(spacing: 5) {
                        Image(systemName: itemState.symbolName)
                            .font(.caption.weight(.semibold))
                            .foregroundStyle(itemState.color)
                            .widgetAccentable()
                            .frame(width: 14)
                        Text(itemState.name)
                            .font(.caption2)
                            .lineLimit(1)
                        Spacer(minLength: 2)
                        Text(
                            formatDuration(
                                Double(item.milliseconds) / 1_000,
                                compact: true
                            )
                        )
                        .font(.caption2.weight(.semibold))
                        .monospacedDigit()
                        .foregroundStyle(.secondary)
                    }
                }
            }
            Spacer(minLength: 0)
            Link(destination: URL(string: "timetracker://insights")!) {
                Label("Insights", systemImage: "chart.pie.fill")
                    .font(.caption2.weight(.semibold))
            }
        }
        .frame(maxWidth: .infinity, alignment: .leading)
    }

    private func quickSwitches(excluding currentID: Int) -> some View {
        let ids = (entry.predictedStateIDs + TrackerState.all.map(\.id))
            .filter { $0 != currentID && TrackerState.all.indices.contains($0) }
        let unique = ids.reduce(into: [Int]()) { result, id in
            if !result.contains(id), result.count < 3 {
                result.append(id)
            }
        }
        return HStack(spacing: 6) {
            ForEach(unique, id: \.self) { id in
                let activity = TrackerState.by(id: id)
                Button(intent: SwitchActivityIntent(state: activity)) {
                    Image(systemName: activity.symbolName)
                        .frame(maxWidth: .infinity)
                }
                .buttonStyle(.bordered)
                .tint(activity.color)
                .accessibilityLabel("Switch to \(activity.name)")
            }
        }
    }

    private var emptyView: some View {
        VStack(spacing: 6) {
            Image(systemName: entry.isConfigured ? "timer" : "link.badge.plus")
                .font(.title2)
                .widgetAccentable()
            Text(entry.isConfigured ? "Nothing tracked yet" : "Open TimeTracker to connect")
                .font(.caption)
                .multilineTextAlignment(.center)
                .foregroundStyle(.secondary)
        }
    }

    @ViewBuilder
    private var updatedLabel: some View {
        if let lastUpdated = entry.lastUpdated {
            Text("Updated \(lastUpdated, style: .relative)")
                .font(.system(size: 9))
                .foregroundStyle(.secondary)
                .lineLimit(1)
        }
    }

    private var state: TrackerState? {
        entry.currentStateID.flatMap { id in
            TrackerState.all.indices.contains(id) ? TrackerState.by(id: id) : nil
        }
    }

    private var backgroundGradient: some View {
        let tint = state?.color ?? .gray
        return LinearGradient(
            colors: [tint.opacity(0.32), tint.opacity(0.06)],
            startPoint: .topLeading,
            endPoint: .bottomTrailing
        )
    }
}

struct TimeTrackerWidgetPushHandler: WidgetPushHandler {
    nonisolated init() {}

    nonisolated func pushTokenDidChange(
        _ pushInfo: WidgetPushInfo,
        widgets: [WidgetInfo]
    ) {
        guard widgets.contains(where: { $0.kind == CurrentActivityWidget.kind }) else {
            return
        }
        Task { @MainActor in
            guard let config = ServerConfig.load() else { return }
            let bundleID = Bundle.main.bundleIdentifier
                ?? "at.janez.TimeTracker.Widgets"
            try? await APIClient(config: config).registerWidgetPushToken(
                pushInfo.token,
                topic: "\(bundleID).push-type.widgets",
                environment: pushEnvironment
            )
        }
    }

    @MainActor
    private var pushEnvironment: String {
        #if DEBUG
        "sandbox"
        #else
        "production"
        #endif
    }
}

struct CurrentActivityWidget: Widget {
    nonisolated static let kind = "at.janez.TimeTracker.current"

    var body: some WidgetConfiguration {
        StaticConfiguration(
            kind: Self.kind,
            provider: ActivityProvider()
        ) { entry in
            CurrentActivityWidgetView(entry: entry)
        }
        .configurationDisplayName("Current Activity")
        .description("See and switch the activity you're tracking.")
        .supportedFamilies([
            .systemSmall,
            .systemMedium,
            .accessoryCircular,
            .accessoryRectangular,
            .accessoryInline,
        ])
        .pushHandler(TimeTrackerWidgetPushHandler.self)
    }
}
