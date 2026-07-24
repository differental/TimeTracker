import Charts
import SwiftUI

struct InsightsView: View {
    @Environment(AppModel.self) private var model
    @Environment(\.accessibilityReduceMotion) private var reduceMotion
    @State private var selectedShare: StateShare?

    private let ranges: [(label: String, days: Int)] = [
        ("Today", 1),
        ("Week", 7),
        ("Month", 30),
    ]

    private var shares: [StateShare] {
        model.summary.filter { $0.milliseconds > 0 }
    }

    var body: some View {
        NavigationStack {
            ScrollView {
                VStack(spacing: 24) {
                    rangePicker

                    if shares.isEmpty {
                        ContentUnavailableView(
                            "No data yet",
                            systemImage: "chart.pie",
                            description: Text("Tracked time will show up here.")
                        )
                        .padding(.top, 60)
                    } else {
                        donut
                        AIDigestCard(
                            shares: shares,
                            entries: model.analyticsEntries,
                            days: model.summaryDays
                        )
                        breakdown
                    }
                }
                .padding()
            }
            .navigationTitle("Insights")
            .refreshable { await model.refresh() }
            .onChange(of: model.summaryDays) {
                selectedShare = nil
            }
        }
    }

    private var rangePicker: some View {
        @Bindable var model = model
        return Picker(
            "Range",
            selection: Binding(
                get: { model.summaryDays },
                set: { days in Task { await model.setSummaryDays(days) } }
            )
        ) {
            ForEach(ranges, id: \.days) { range in
                Text(range.label).tag(range.days)
            }
        }
        .pickerStyle(.segmented)
    }

    private var donut: some View {
        Chart(shares) { share in
            SectorMark(
                angle: .value("Time", share.duration),
                innerRadius: .ratio(0.62),
                angularInset: 1.5
            )
            .cornerRadius(5)
            .foregroundStyle(share.state.color)
            .opacity(selectedShare == nil || selectedShare?.id == share.id ? 1 : 0.35)
            .accessibilityLabel(share.state.name)
            .accessibilityValue(formatDuration(share.duration, compact: true))
        }
        .chartAngleSelection(
            value: Binding(
                get: { nil as TimeInterval? },
                set: { value in
                    guard let value else { return }
                    var running: TimeInterval = 0
                    selectedShare = shares.first { share in
                        running += share.duration
                        return value <= running
                    }
                }
            )
        )
        .frame(height: 260)
        .chartBackground { proxy in
            GeometryReader { geometry in
                if let frame = proxy.plotFrame.map({ geometry[$0] }) {
                    let focus = selectedShare
                    VStack(spacing: 4) {
                        Group {
                            if let focus {
                                Label(focus.state.name, systemImage: focus.state.symbolName)
                                    .symbolRenderingMode(.hierarchical)
                                    .foregroundStyle(focus.state.color)
                            } else {
                                Text("Tracked")
                            }
                        }
                        .font(.system(.callout, design: .rounded, weight: .medium))
                        Text(formatDuration(focus?.duration ?? model.totalTracked, compact: true))
                            .font(.system(.title, design: .rounded, weight: .bold))
                            .contentTransition(.numericText())
                    }
                    .position(x: frame.midX, y: frame.midY)
                }
            }
        }
        .accessibilityLabel("Tracked time by activity")
        .accessibilityValue(
            shares
                .map {
                    "\($0.state.name), \(formatDuration($0.duration, compact: true))"
                }
                .joined(separator: "; ")
        )
        .animation(reduceMotion ? nil : .smooth, value: selectedShare?.id)
    }

    private var breakdown: some View {
        VStack(spacing: 10) {
            ForEach(shares) { share in
                let fraction = model.totalTracked > 0
                    ? share.duration / model.totalTracked
                    : 0
                Button {
                    selectedShare = selectedShare?.id == share.id ? nil : share
                } label: {
                    HStack(spacing: 12) {
                        Image(systemName: share.state.symbolName)
                            .symbolRenderingMode(.hierarchical)
                            .foregroundStyle(share.state.color)
                            .frame(width: 22)
                        VStack(alignment: .leading, spacing: 4) {
                            HStack {
                                Text(share.state.name)
                                    .font(.system(.subheadline, design: .rounded, weight: .medium))
                                Spacer()
                                Text(formatDuration(share.duration, compact: true))
                                    .font(.system(.subheadline, design: .rounded, weight: .semibold))
                                    .monospacedDigit()
                                Text(
                                    fraction.formatted(
                                        .percent.precision(.fractionLength(0))
                                    )
                                )
                                .font(.caption)
                                .foregroundStyle(.secondary)
                                .frame(width: 42, alignment: .trailing)
                            }
                            GeometryReader { geometry in
                                Capsule()
                                    .fill(share.state.color.opacity(0.25))
                                    .overlay(alignment: .leading) {
                                        Capsule()
                                            .fill(share.state.color)
                                            .frame(
                                                width: max(
                                                    4,
                                                    geometry.size.width * fraction
                                                )
                                            )
                                    }
                            }
                            .frame(height: 6)
                        }
                    }
                    .padding(12)
                    .contentShape(RoundedRectangle(cornerRadius: 14))
                }
                .buttonStyle(.plain)
                .glassEffect(.regular, in: RoundedRectangle(cornerRadius: 14))
            }
        }
    }
}

#Preview {
    InsightsView()
        .environment(AppModel())
}
