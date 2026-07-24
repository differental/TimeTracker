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

                ScrollView {
                    VStack(spacing: 20) {
                        hero
                            .padding(.horizontal)

                        if let error = actionError ?? model.lastError {
                            errorBanner(error)
                                .padding(.horizontal)
                        }

                        Button {
                            showFocusSession = true
                        } label: {
                            Label("Start Focus Session", systemImage: "timer.circle")
                                .font(.headline)
                                .frame(maxWidth: .infinity)
                                .frame(height: 54)
                        }
                        .buttonStyle(.plain)
                        .glassEffect(
                            .regular.interactive(),
                            in: RoundedRectangle(cornerRadius: 18)
                        )
                        .padding(.horizontal)

                        predictedActions
                            .padding(.horizontal)
                    }
                    .padding(.vertical)
                    .padding(.bottom, 90)
                }
                .scrollIndicators(.hidden)
                .refreshable { await model.refresh() }
            }
            .navigationTitle("TimeTracker")
            .toolbarBackground(.hidden, for: .navigationBar)
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

    private var hero: some View {
        let current = model.current
        let state = current?.state

        return VStack(spacing: 12) {
            Spacer(minLength: 0)
            if let state {
                Label(state.name, systemImage: state.symbolName)
                    .font(.system(.title, design: .rounded, weight: .semibold))
                    .symbolRenderingMode(.hierarchical)
                    .foregroundStyle(.white)
                    .contentTransition(.opacity)
                    .accessibilityLabel(state.name)
                    .accessibilityIdentifier("currentActivityName")

                ElapsedTimer(since: current?.startDate ?? .now)

                Text("since \(current?.startDate.formatted(date: .abbreviated, time: .shortened) ?? "")")
                    .font(.footnote)
                    .foregroundStyle(.white.opacity(0.75))
            } else {
                Image(systemName: "timer")
                    .font(.system(size: 34, weight: .semibold))
                    .symbolRenderingMode(.hierarchical)
                    .foregroundStyle(.white)
                Text("Nothing tracked yet")
                    .font(.system(.title2, design: .rounded, weight: .semibold))
                    .foregroundStyle(.white)
                Text(model.isConfigured
                     ? "Pick an activity below to start."
                     : "Connect to your server in Settings.")
                    .font(.footnote)
                    .foregroundStyle(.white.opacity(0.75))
            }
            Spacer(minLength: 0)
        }
        .padding(28)
        .frame(maxWidth: .infinity)
        .frame(height: 240)
        .background(.black.opacity(0.24), in: RoundedRectangle(cornerRadius: 28))
        .overlay {
            RoundedRectangle(cornerRadius: 28)
                .strokeBorder(.white.opacity(0.14), lineWidth: 1)
        }
        .animation(.smooth, value: model.current?.stateID)
    }

    private var predictedActions: some View {
        GlassEffectContainer(spacing: 12) {
            VStack(spacing: 12) {
                ForEach(Array(model.predictedStates.enumerated()), id: \.element.id) {
                    index,
                    state in
                    predictedButton(state, slot: index)
                }

                Button {
                    showAllActivities = true
                } label: {
                    Label("More", systemImage: "ellipsis")
                        .font(.headline)
                        .frame(maxWidth: .infinity)
                        .frame(height: 54)
                        .contentShape(.rect)
                }
                .buttonStyle(.plain)
                .glassEffect(
                    .regular.interactive(),
                    in: RoundedRectangle(cornerRadius: 18, style: .continuous)
                )
                .disabled(switchingStateID != nil)
                .accessibilityIdentifier("moreActivities")
            }
        }
    }

    private func predictedButton(_ state: TrackerState, slot: Int) -> some View {
        let shape = RoundedRectangle(cornerRadius: 20, style: .continuous)

        return Button {
            switchActivity(to: state, dismissMoreOnSuccess: false)
        } label: {
            HStack(spacing: 14) {
                Image(systemName: state.symbolName)
                    .font(.system(size: 28, weight: .semibold))
                    .symbolRenderingMode(.hierarchical)
                Text(state.name)
                    .font(.system(.title3, design: .rounded, weight: .semibold))
                    .lineLimit(1)
                    .minimumScaleFactor(0.7)
                Spacer()
                if switchingStateID == state.id {
                    ProgressView()
                        .tint(.white)
                } else {
                    Image(systemName: "arrow.right")
                        .font(.subheadline.weight(.bold))
                        .foregroundStyle(.white.opacity(0.7))
                }
            }
            .foregroundStyle(.white)
            .padding(.horizontal, 20)
            .frame(maxWidth: .infinity)
            .frame(height: 78)
            .contentShape(.rect)
        }
        .buttonStyle(.plain)
        .glassEffect(
            .regular.tint(state.color.opacity(0.13)).interactive(),
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
        LinearGradient(
            colors: [tint.opacity(0.55), tint.opacity(0.18), .black.opacity(0.5)],
            startPoint: .topLeading,
            endPoint: .bottomTrailing
        )
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
    @ScaledMetric(relativeTo: .largeTitle) private var fontSize = 56.0

    var body: some View {
        TimelineView(.periodic(from: .now, by: 1)) { timeline in
            let elapsed = timeline.date.timeIntervalSince(since)
            Text(formatDuration(elapsed))
                .font(.system(size: fontSize, weight: .bold, design: .rounded))
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
