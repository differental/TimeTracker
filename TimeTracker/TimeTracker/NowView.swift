import Combine
import SwiftUI

struct NowView: View {
    @Environment(AppModel.self) private var model
    @State private var showSettings = false
    @State private var showAllActivities = false
    @State private var showFocusSession = false
    @State private var switchingStateID: Int?
    @State private var actionError: String?

    var body: some View {
        NavigationStack {
            ZStack {
                AuroraBackground(tint: model.current?.state.color ?? .gray)

                GeometryReader { proxy in
                    let layout = NowLayoutMetrics(size: proxy.size)

                    Group {
                        if layout.usesSideBySideLayout {
                            HStack(spacing: layout.sectionSpacing) {
                                hero(
                                    height: layout.heroHeight,
                                    compact: true
                                )

                                controls(layout: layout)
                            }
                        } else {
                            VStack(spacing: layout.sectionSpacing) {
                                hero(
                                    height: layout.heroHeight,
                                    compact: layout.isCompact
                                )

                                controls(layout: layout)
                            }
                        }
                    }
                    .padding(.horizontal, layout.edgeInset)
                    .padding(.vertical, layout.edgeInset)
                    .frame(
                        maxWidth: .infinity,
                        maxHeight: .infinity,
                        alignment: .top
                    )
                }
            }
            .navigationTitle("TimeTracker")
            .navigationBarTitleDisplayMode(.inline)
            .toolbarBackground(.hidden, for: .navigationBar)
            .toolbarColorScheme(.dark, for: .navigationBar)
            .toolbar {
                ToolbarItem(placement: .topBarTrailing) {
                    Button("Settings", systemImage: "gearshape") {
                        showSettings = true
                    }
                }
            }
            .sheet(isPresented: $showSettings) {
                SettingsView()
            }
            .sheet(isPresented: $showAllActivities) {
                allActivitiesSheet
                    .presentationDetents([.medium, .large])
            }
            .sheet(isPresented: $showFocusSession) {
                FocusSessionSheet()
                    .presentationDetents([.medium, .large])
            }
            .task {
                await refreshPredictionsAtHourBoundaries()
            }
        }
    }

    private func controls(layout: NowLayoutMetrics) -> some View {
        VStack(spacing: layout.sectionSpacing) {
            if let error = actionError ?? model.lastError {
                errorBanner(error)
            }

            Button {
                showFocusSession = true
            } label: {
                Label("Start Focus Session", systemImage: "timer")
                    .font(.subheadline.weight(.semibold))
                    .foregroundStyle(.white)
                    .frame(maxWidth: .infinity)
                    .frame(height: layout.focusButtonHeight)
                    .contentShape(.rect)
            }
            .buttonStyle(.plain)
            .glassEffect(
                .regular.interactive(),
                in: RoundedRectangle(cornerRadius: layout.cornerRadius)
            )

            predictedActions(layout: layout)
        }
        .frame(maxWidth: .infinity)
    }

    private func hero(height: CGFloat, compact: Bool) -> some View {
        let current = model.current
        let state = current?.state

        return VStack(spacing: compact ? 5 : 9) {
            Spacer(minLength: 0)
            if let state {
                HStack(spacing: compact ? 8 : 10) {
                    Image(systemName: state.symbolName)
                        .font(.system(size: compact ? 12 : 14, weight: .semibold))
                        .foregroundStyle(.white)
                        .frame(
                            width: compact ? 26 : 30,
                            height: compact ? 26 : 30
                        )
                        .background(state.color.gradient, in: Circle())
                        .shadow(color: state.color.opacity(0.45), radius: 7, y: 2)
                    Text(state.name)
                        .font(
                            .system(
                                compact ? .headline : .title3,
                                design: .rounded,
                                weight: .semibold
                            )
                        )
                        .foregroundStyle(.white)
                }
                .contentTransition(.opacity)
                .accessibilityElement(children: .ignore)
                .accessibilityLabel(state.name)
                .accessibilityIdentifier("currentActivityName")

                ElapsedTimer(
                    since: current?.startDate ?? .now,
                    compact: compact
                )

                Text(sinceText(for: current?.startDate ?? .now))
                    .font((compact ? Font.caption2 : .caption).weight(.medium))
                    .foregroundStyle(.white.opacity(0.85))
                    .lineLimit(1)
                    .padding(.horizontal, 10)
                    .padding(.vertical, compact ? 3 : 4)
                    .background(.white.opacity(0.12), in: Capsule())
            } else {
                Image(systemName: "timer")
                    .font(.system(size: compact ? 24 : 30, weight: .semibold))
                    .symbolRenderingMode(.hierarchical)
                    .foregroundStyle(.white)
                Text("Nothing tracked yet")
                    .font(
                        .system(
                            compact ? .headline : .title3,
                            design: .rounded,
                            weight: .semibold
                        )
                    )
                    .foregroundStyle(.white)
                Text(model.isConfigured
                     ? "Pick an activity below to start."
                     : "Connect to your server in Settings.")
                    .font(compact ? .caption2 : .caption)
                    .foregroundStyle(.white.opacity(0.75))
                    .lineLimit(1)
                    .minimumScaleFactor(0.8)
            }
            Spacer(minLength: 0)
        }
        .padding(.horizontal, compact ? 16 : 20)
        .padding(.vertical, compact ? 10 : 14)
        .frame(maxWidth: .infinity)
        .frame(height: height)
        .background(
            .black.opacity(0.22),
            in: RoundedRectangle(cornerRadius: 24, style: .continuous)
        )
        .overlay {
            RoundedRectangle(cornerRadius: 24, style: .continuous)
                .strokeBorder(
                    LinearGradient(
                        colors: [.white.opacity(0.28), .white.opacity(0.05)],
                        startPoint: .top,
                        endPoint: .bottom
                    ),
                    lineWidth: 1
                )
        }
        .animation(.smooth, value: model.current?.stateID)
    }

    private func sinceText(for date: Date) -> String {
        if Calendar.current.isDateInToday(date) {
            return "since \(date.formatted(date: .omitted, time: .shortened))"
        }
        return "since \(date.formatted(date: .abbreviated, time: .shortened))"
    }

    private func predictedActions(layout: NowLayoutMetrics) -> some View {
        GlassEffectContainer(spacing: layout.controlSpacing) {
            VStack(spacing: layout.controlSpacing) {
                if !layout.isCompact {
                    Text("SUGGESTED")
                        .font(.caption2.weight(.semibold))
                        .kerning(1.4)
                        .foregroundStyle(.white.opacity(0.55))
                        .frame(maxWidth: .infinity, alignment: .leading)
                        .padding(.leading, 6)
                        .padding(.top, 2)
                        .accessibilityHidden(true)
                }

                ForEach(Array(model.predictedStates.enumerated()), id: \.element.id) {
                    index,
                    state in
                    predictedButton(state, slot: index, layout: layout)
                }

                Button {
                    showAllActivities = true
                } label: {
                    Label("All activities", systemImage: "square.grid.2x2")
                        .font(.subheadline.weight(.semibold))
                        .foregroundStyle(.white.opacity(0.9))
                        .frame(maxWidth: .infinity)
                        .frame(height: layout.moreButtonHeight)
                        .contentShape(.rect)
                }
                .buttonStyle(.plain)
                .glassEffect(
                    .regular.interactive(),
                    in: RoundedRectangle(
                        cornerRadius: layout.cornerRadius,
                        style: .continuous
                    )
                )
                .disabled(switchingStateID != nil)
                .accessibilityIdentifier("moreActivities")
            }
        }
    }

    private func predictedButton(
        _ state: TrackerState,
        slot: Int,
        layout: NowLayoutMetrics
    ) -> some View {
        let shape = RoundedRectangle(
            cornerRadius: layout.cornerRadius,
            style: .continuous
        )

        let badgeSize: CGFloat = layout.isCompact ? 30 : 36

        return Button {
            switchActivity(to: state, dismissMoreOnSuccess: false)
        } label: {
            HStack(spacing: layout.isCompact ? 10 : 12) {
                ZStack {
                    Circle()
                        .fill(state.color.gradient)
                    if switchingStateID == state.id {
                        ProgressView()
                            .tint(.white)
                            .controlSize(.small)
                    } else {
                        Image(systemName: state.symbolName)
                            .font(
                                .system(
                                    size: layout.isCompact ? 13 : 15,
                                    weight: .semibold
                                )
                            )
                            .foregroundStyle(.white)
                    }
                }
                .frame(width: badgeSize, height: badgeSize)
                .shadow(color: state.color.opacity(0.35), radius: 5, y: 2)
                Text(state.name)
                    .font(
                        .system(
                            layout.isCompact ? .body : .headline,
                            design: .rounded,
                            weight: .semibold
                        )
                    )
                    .lineLimit(1)
                    .minimumScaleFactor(0.7)
                Spacer()
            }
            .foregroundStyle(.white)
            .padding(.horizontal, layout.isCompact ? 12 : 14)
            .frame(maxWidth: .infinity)
            .frame(height: layout.activityButtonHeight)
            .contentShape(.rect)
        }
        .buttonStyle(.plain)
        .glassEffect(
            .regular.tint(state.color.opacity(0.2)).interactive(),
            in: shape
        )
        .disabled(switchingStateID != nil)
        .accessibilityLabel("Switch to \(state.name)")
        .accessibilityIdentifier("predictedActivity.\(slot)")
    }

    private var allActivitiesSheet: some View {
        NavigationStack {
            List {
                if let actionError {
                    Section {
                        Label(
                            actionError,
                            systemImage: "exclamationmark.triangle.fill"
                        )
                        .font(.footnote)
                        .foregroundStyle(.orange)
                    }
                }

                ForEach(TrackerState.all) { state in
                    let isCurrent = model.current?.stateID == state.id
                    Button {
                        switchActivity(to: state, dismissMoreOnSuccess: true)
                    } label: {
                        HStack(spacing: 12) {
                            Image(systemName: state.symbolName)
                                .font(.title3)
                                .symbolRenderingMode(.hierarchical)
                                .foregroundStyle(state.color)
                                .frame(width: 30)
                            Text(state.name)
                                .foregroundStyle(.primary)
                            Spacer()
                            if isCurrent {
                                Text("Current")
                                    .font(.caption.weight(.semibold))
                                    .foregroundStyle(.secondary)
                            } else if switchingStateID == state.id {
                                ProgressView()
                            }
                        }
                        .contentShape(.rect)
                    }
                    .buttonStyle(.plain)
                    .disabled(isCurrent || switchingStateID != nil)
                    .accessibilityLabel(
                        isCurrent ? "\(state.name), current activity" : "Switch to \(state.name)"
                    )
                    .accessibilityIdentifier("moreActivity.\(state.id)")
                }
            }
            .navigationTitle("All Activities")
            .navigationBarTitleDisplayMode(.inline)
            .toolbar {
                ToolbarItem(placement: .cancellationAction) {
                    Button("Done") {
                        showAllActivities = false
                    }
                    .disabled(switchingStateID != nil)
                }
            }
        }
    }

    private func switchActivity(
        to state: TrackerState,
        dismissMoreOnSuccess: Bool
    ) {
        guard switchingStateID == nil else { return }
        switchingStateID = state.id
        actionError = nil
        Task {
            do {
                try await model.switchState(to: state, at: .now)
                if dismissMoreOnSuccess {
                    showAllActivities = false
                }
            } catch {
                actionError = error.localizedDescription
            }
            switchingStateID = nil
        }
    }

    private func refreshPredictionsAtHourBoundaries() async {
        while !Task.isCancelled {
            let calendar = Calendar.current
            guard let nextHour = calendar.nextDate(
                after: .now,
                matching: DateComponents(minute: 0, second: 0),
                matchingPolicy: .nextTime
            ) else { return }
            do {
                try await Task.sleep(
                    for: .seconds(max(1, nextHour.timeIntervalSinceNow))
                )
            } catch {
                return
            }
            model.recomputePredictions()
        }
    }

    private func errorBanner(_ message: String) -> some View {
        Label(message, systemImage: "exclamationmark.triangle.fill")
            .font(.footnote)
            .foregroundStyle(.orange)
            .frame(maxWidth: .infinity, alignment: .leading)
            .padding(12)
            .background(.regularMaterial, in: RoundedRectangle(cornerRadius: 14))
    }
}

private struct NowLayoutMetrics {
    let size: CGSize

    var usesSideBySideLayout: Bool {
        size.width > size.height && size.width >= 640
    }

    var isCompact: Bool {
        usesSideBySideLayout || size.height < 600
    }

    var edgeInset: CGFloat {
        isCompact ? 10 : 16
    }

    var sectionSpacing: CGFloat {
        isCompact ? 8 : 14
    }

    var controlSpacing: CGFloat {
        isCompact ? 6 : 10
    }

    var cornerRadius: CGFloat {
        isCompact ? 15 : 18
    }

    var heroHeight: CGFloat {
        if usesSideBySideLayout {
            return max(150, size.height - edgeInset * 2)
        }
        if isCompact {
            return min(128, max(110, size.height * 0.25))
        }
        return min(216, max(150, size.height * 0.30))
    }

    var focusButtonHeight: CGFloat {
        isCompact ? 42 : 48
    }

    var activityButtonHeight: CGFloat {
        isCompact ? 44 : 56
    }

    var moreButtonHeight: CGFloat {
        isCompact ? 40 : 46
    }
}

private struct FocusSessionSheet: View {
    @Environment(AppModel.self) private var model
    @Environment(\.dismiss) private var dismiss

    @State private var stateID = 1
    @State private var minutes = 45
    @State private var isStarting = false
    @State private var errorMessage: String?

    var body: some View {
        NavigationStack {
            Form {
                Section("Activity") {
                    Picker("Activity", selection: $stateID) {
                        ForEach(TrackerState.all) { state in
                            Label(state.name, systemImage: state.symbolName)
                                .tag(state.id)
                        }
                    }
                }
                Section {
                    Picker("Minutes", selection: $minutes) {
                        ForEach([15, 25, 45, 60, 90, 120], id: \.self) {
                            Text("\($0) minutes").tag($0)
                        }
                    }
                    .pickerStyle(.wheel)
                } header: {
                    Text("Duration")
                } footer: {
                    Text(
                        "TimeTracker uses an AlarmKit timer that appears on the Lock Screen, Dynamic Island, StandBy, and paired Apple Watch."
                    )
                }
                if let errorMessage {
                    Section {
                        Label(errorMessage, systemImage: "exclamationmark.triangle")
                            .foregroundStyle(.orange)
                    }
                }
            }
            .navigationTitle("Focus Session")
            .navigationBarTitleDisplayMode(.inline)
            .toolbar {
                ToolbarItem(placement: .cancellationAction) {
                    Button("Cancel", role: .cancel) { dismiss() }
                }
                ToolbarItem(placement: .confirmationAction) {
                    Button("Start") { start() }
                        .disabled(isStarting)
                }
            }
        }
        .onAppear {
            stateID = model.predictedStates.first?.id ?? 1
        }
    }

    private func start() {
        guard !isStarting else { return }
        isStarting = true
        errorMessage = nil
        Task {
            do {
                try await model.startFocusSession(
                    activity: TrackerState.by(id: stateID),
                    durationMinutes: minutes
                )
                dismiss()
            } catch {
                errorMessage = error.localizedDescription
                isStarting = false
            }
        }
    }
}

private struct AuroraBackground: View {
    let tint: Color
    @Environment(\.accessibilityReduceMotion) private var reduceMotion
    @State private var lowPowerMode = ProcessInfo.processInfo.isLowPowerModeEnabled

    var body: some View {
        Group {
            if reduceMotion || lowPowerMode {
                staticBackground
            } else {
                TimelineView(.animation(minimumInterval: 1.0 / 20.0)) { timeline in
                    renderedBackground(at: timeline.date)
                }
            }
        }
        .onReceive(
            NotificationCenter.default.publisher(
                for: .NSProcessInfoPowerStateDidChange
            )
        ) { _ in
            lowPowerMode = ProcessInfo.processInfo.isLowPowerModeEnabled
        }
        .ignoresSafeArea()
        .animation(reduceMotion ? nil : .smooth, value: tint)
        .accessibilityHidden(true)
    }

    private var staticBackground: some View {
        ZStack {
            Color(red: 0.016, green: 0.02, blue: 0.048)
            LinearGradient(
                colors: [tint.opacity(0.6), tint.opacity(0.15), .clear],
                startPoint: .topLeading,
                endPoint: .bottomTrailing
            )
        }
    }

    private func renderedBackground(at date: Date) -> some View {
        let time = date.timeIntervalSinceReferenceDate
        return Rectangle()
            .colorEffect(
                ShaderLibrary.aurora(
                    .boundingRect,
                    .float(time.truncatingRemainder(dividingBy: 100_000)),
                    .color(tint)
                )
            )
            .overlay {
                LinearGradient(
                    colors: [
                        .black.opacity(0.02),
                        .black.opacity(0.16),
                        .black.opacity(0.34),
                    ],
                    startPoint: .top,
                    endPoint: .bottom
                )
            }
    }
}

struct ElapsedTimer: View {
    let since: Date
    let compact: Bool
    @ScaledMetric(relativeTo: .largeTitle) private var fontSize = 48.0

    var body: some View {
        TimelineView(.periodic(from: .now, by: 1)) { timeline in
            let elapsed = timeline.date.timeIntervalSince(since)
            Text(formatDuration(elapsed))
                .font(
                    .system(
                        size: compact ? fontSize * 0.82 : fontSize,
                        weight: .bold,
                        design: .rounded
                    )
                )
                .monospacedDigit()
                .foregroundStyle(.white)
                .contentTransition(.numericText())
                .minimumScaleFactor(0.55)
                .accessibilityElement(children: .ignore)
                .accessibilityLabel("Elapsed time")
                .accessibilityValue(formatDuration(elapsed, compact: true))
        }
    }
}

#Preview {
    NowView()
        .environment(AppModel())
}
