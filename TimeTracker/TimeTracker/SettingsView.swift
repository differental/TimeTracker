import SwiftUI

struct SettingsView: View {
    @Environment(AppModel.self) private var model
    @Environment(\.dismiss) private var dismiss

    @State private var testResult: TestResult?
    @State private var isTesting = false
    @State private var predictionTestResult: PredictionTestResult?
    @State private var isTestingPredictions = false

    enum TestResult {
        case success(entries: UInt64)
        case failure(String)
    }

    enum PredictionTestResult {
        case success(ActivityPredictor.BacktestResult)
        case failure(String)
    }

    var body: some View {
        @Bindable var model = model
        NavigationStack {
            Form {
                Section {
                    TextField(
                        "https://tracker.example.com",
                        text: $model.serverURLString
                    )
                    .keyboardType(.URL)
                    .textContentType(.URL)
                    .autocorrectionDisabled()
                    .textInputAutocapitalization(.never)
                } header: {
                    Text("Server URL")
                } footer: {
                    Text("The address of your TimeTracker server.")
                }

                Section {
                    SecureField("Access key", text: $model.accessKey)
                        .textContentType(.password)
                } header: {
                    Text("Access key")
                } footer: {
                    Text("Stored securely in the Keychain and sent with every request.")
                }

                Section {
                    Button {
                        test()
                    } label: {
                        if isTesting {
                            HStack(spacing: 8) {
                                ProgressView()
                                Text("Testing…")
                            }
                        } else {
                            Label("Test connection", systemImage: "bolt.horizontal")
                        }
                    }
                    .disabled(isTesting || isTestingPredictions)

                    switch testResult {
                    case .success(let entries):
                        Label(
                            "Connected — \(entries) entries on server",
                            systemImage: "checkmark.circle.fill"
                        )
                        .foregroundStyle(.green)
                    case .failure(let message):
                        Label(message, systemImage: "xmark.circle.fill")
                            .foregroundStyle(.red)
                    case nil:
                        EmptyView()
                    }
                }

                Section {
                    Toggle(
                        "Use upcoming Calendar events",
                        isOn: Binding(
                            get: { model.calendarSuggestionsEnabled },
                            set: { enabled in
                                Task {
                                    await model.setCalendarSuggestionsEnabled(enabled)
                                }
                            }
                        )
                    )
                } header: {
                    Text("Suggestions")
                } footer: {
                    Text(
                        "Optional full Calendar access improves the three suggested activities. Event titles are processed on device and are never sent to the TimeTracker server."
                    )
                }

                Section {
                    settingSlider(
                        "History tables",
                        value: $model.predictionHistoryTableCount,
                        range: 1...6,
                        step: 1
                    )
                    settingSlider(
                        "Base table entries",
                        value: $model.predictionBaseTableSize,
                        range: 128...2_048,
                        step: 128
                    )
                    settingSlider(
                        "Tagged table entries",
                        value: $model.predictionTaggedTableSize,
                        range: 128...2_048,
                        step: 128
                    )
                    settingSlider(
                        "Usefulness aging",
                        value: $model.predictionUsefulnessAgingInterval,
                        range: 64...1_024,
                        step: 64
                    )
                    settingSlider(
                        "Replay window",
                        value: $model.predictionReplayWindowSize,
                        range: 25...1_000,
                        step: 25
                    )
                } header: {
                    Text("TAGE prediction")
                } footer: {
                    Text(
                        "Table sizes trade memory for capacity. The aging interval controls how quickly stale tagged entries lose influence. The replay window is the number of earlier entries available for each accuracy prediction."
                    )
                }
                .disabled(isTestingPredictions)

                Section {
                    Button {
                        testPredictions()
                    } label: {
                        if isTestingPredictions {
                            HStack(spacing: 8) {
                                ProgressView()
                                Text("Replaying history…")
                            }
                        } else {
                            Label(
                                "Test prediction accuracy",
                                systemImage: "gauge.with.dots.needle.67percent"
                            )
                        }
                    }
                    .disabled(isTestingPredictions || isTesting)

                    switch predictionTestResult {
                    case .success(let result) where result.evaluatedTransitions > 0:
                        LabeledContent(
                            "Overall accuracy",
                            value: result.firstChoiceAccuracy.formatted(
                                .percent.precision(.fractionLength(1))
                            )
                        )
                        LabeledContent(
                            "Top-three accuracy",
                            value: result.topThreeAccuracy.formatted(
                                .percent.precision(.fractionLength(1))
                            )
                        )
                        Text(
                            "\(result.evaluatedTransitions.formatted()) transitions tested with a \(result.windowSize)-entry sliding window."
                        )
                        .font(.footnote)
                        .foregroundStyle(.secondary)
                    case .success:
                        Label(
                            "At least two entries are needed.",
                            systemImage: "info.circle"
                        )
                        .foregroundStyle(.secondary)
                    case .failure(let message):
                        Label(message, systemImage: "xmark.circle.fill")
                            .foregroundStyle(.red)
                    case nil:
                        EmptyView()
                    }
                } footer: {
                    Text(
                        "Each entry is predicted using only older data. Calendar suggestions are excluded so the result measures the on-device TAGE predictor."
                    )
                }
            }
            .navigationTitle("Settings")
            .navigationBarTitleDisplayMode(.inline)
            .toolbar {
                ToolbarItem(placement: .confirmationAction) {
                    Button("Done") {
                        model.saveSettings()
                        dismiss()
                        Task { await model.refresh() }
                    }
                }
            }
        }
    }

    private func test() {
        model.saveSettings()
        isTesting = true
        testResult = nil
        Task {
            defer { isTesting = false }
            guard let client = model.client else {
                testResult = .failure("Enter a valid URL and key first.")
                return
            }
            do {
                let length = try await client.length()
                testResult = .success(entries: length)
            } catch {
                testResult = .failure(error.localizedDescription)
            }
        }
    }

    @ViewBuilder
    private func settingSlider(
        _ title: String,
        value: Binding<Int>,
        range: ClosedRange<Int>,
        step: Int
    ) -> some View {
        VStack(alignment: .leading, spacing: 6) {
            LabeledContent(title, value: value.wrappedValue.formatted())
            Slider(
                value: Binding(
                    get: { Double(value.wrappedValue) },
                    set: {
                        value.wrappedValue = Int($0.rounded())
                        predictionTestResult = nil
                    }
                ),
                in: Double(range.lowerBound)...Double(range.upperBound),
                step: Double(step)
            )
            .accessibilityLabel(title)
            .accessibilityValue(value.wrappedValue.formatted())
        }
    }

    private func testPredictions() {
        model.saveSettings()
        isTestingPredictions = true
        predictionTestResult = nil
        Task {
            defer { isTestingPredictions = false }
            do {
                predictionTestResult = .success(
                    try await model.testPredictionAccuracy()
                )
            } catch {
                predictionTestResult = .failure(error.localizedDescription)
            }
        }
    }
}

#Preview {
    SettingsView()
        .environment(AppModel())
}
