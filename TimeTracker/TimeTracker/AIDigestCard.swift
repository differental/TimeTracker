import Foundation
import FoundationModels
import SwiftUI

@Generable
private struct DigestOutput {
    @Guide(description: "A concrete two-sentence observation using exact supplied facts.")
    var observation: String

    @Guide(description: "One short, optional-looking suggestion grounded in the supplied facts.")
    var suggestion: String
}

@Generable
private struct TimeAnalyticsArguments {
    @Guide(description: "First local date to analyze, formatted YYYY-MM-DD.")
    var startDate: String

    @Guide(description: "Last local date to analyze, formatted YYYY-MM-DD.")
    var endDate: String

    @Guide(
        description: "One activity name to filter, or All.",
        .anyOf(["All"] + TrackerState.all.map(\.name))
    )
    var activity: String
}

private struct TimeAnalyticsTool: Tool {
    let name = "analyze_time"
    let description = """
        Calculates exact tracked durations and session counts for a local date \
        range. Always use this tool before answering a question about the \
        user's tracked time. Never calculate durations yourself.
        """

    let entries: [EntryRecord]
    let now: Date
    let calendar: Calendar

    init(entries: [EntryRecord], now: Date = .now) {
        self.entries = entries
        self.now = now
        calendar = .current
    }

    func call(arguments: TimeAnalyticsArguments) async throws -> String {
        guard let start = date(arguments.startDate),
              let inclusiveEnd = date(arguments.endDate),
              let end = calendar.date(byAdding: .day, value: 1, to: inclusiveEnd),
              start < end
        else {
            return "Invalid date range. Dates must use YYYY-MM-DD."
        }

        let requestedState = TrackerState.all.first {
            $0.name.caseInsensitiveCompare(arguments.activity) == .orderedSame
        }
        var totals = Array(repeating: TimeInterval.zero, count: TrackerState.all.count)
        var sessions = Array(repeating: 0, count: TrackerState.all.count)
        var firstStarts = [Int: Date]()
        var lastEnds = [Int: Date]()

        for entry in entries {
            let entryStart = entry.startDate
            let entryEnd = entry.endDate ?? now
            let overlapStart = max(entryStart, start)
            let overlapEnd = min(entryEnd, end)
            guard overlapEnd > overlapStart,
                  requestedState == nil || requestedState?.id == entry.stateID
            else { continue }

            totals[entry.stateID] += overlapEnd.timeIntervalSince(overlapStart)
            sessions[entry.stateID] += 1
            firstStarts[entry.stateID] = min(firstStarts[entry.stateID] ?? overlapStart, overlapStart)
            lastEnds[entry.stateID] = max(lastEnds[entry.stateID] ?? overlapEnd, overlapEnd)
        }

        let rows = TrackerState.all.compactMap { state -> String? in
            guard totals[state.id] > 0 else { return nil }
            let first = firstStarts[state.id]?.formatted(
                date: .omitted,
                time: .shortened
            ) ?? "unknown"
            let last = lastEnds[state.id]?.formatted(
                date: .omitted,
                time: .shortened
            ) ?? "unknown"
            return """
                \(state.name): \(formatDuration(totals[state.id], compact: true)); \
                sessions \(sessions[state.id]); first \(first); last \(last)
                """
        }
        let total = totals.reduce(0, +)
        return """
            Exact local analysis from \(arguments.startDate) through \(arguments.endDate). \
            Total: \(formatDuration(total, compact: true)). \
            \(rows.isEmpty ? "No tracked entries." : rows.joined(separator: " | "))
            """
    }

    private func date(_ value: String) -> Date? {
        let parts = value.split(separator: "-").compactMap { Int($0) }
        guard parts.count == 3 else { return nil }
        return calendar.date(
            from: DateComponents(year: parts[0], month: parts[1], day: parts[2])
        )
    }
}

struct AIDigestCard: View {
    let shares: [StateShare]
    let entries: [EntryRecord]
    let days: Int

    @State private var digest: DigestOutput?
    @State private var question = ""
    @State private var answer: String?
    @State private var isThinking = false
    @State private var errorMessage: String?

    private let suggestions = [
        "Where did most of my time go?",
        "When do I usually start work?",
        "Which activity is most fragmented?",
    ]

    var body: some View {
        VStack(alignment: .leading, spacing: 14) {
            HStack(spacing: 10) {
                Image(systemName: "sparkles")
                    .font(.system(size: 13, weight: .semibold))
                    .foregroundStyle(.white)
                    .frame(width: 28, height: 28)
                    .background(
                        LinearGradient(
                            colors: [.purple, .indigo],
                            startPoint: .topLeading,
                            endPoint: .bottomTrailing
                        ),
                        in: Circle()
                    )
                VStack(alignment: .leading, spacing: 1) {
                    Text("Ask your time")
                        .font(.system(.headline, design: .rounded, weight: .semibold))
                    Text(modelAvailable ? "Apple Intelligence · On device" : "Local analysis")
                        .font(.caption2)
                        .foregroundStyle(.secondary)
                }
                Spacer()
            }

            if let digest {
                VStack(alignment: .leading, spacing: 6) {
                    Text(digest.observation)
                    Text(digest.suggestion)
                        .foregroundStyle(.secondary)
                }
                .font(.callout)
                .contentTransition(.opacity)
            } else {
                Button {
                    Task { await generateDigest() }
                } label: {
                    Label("Generate digest", systemImage: "text.sparkle")
                        .font(.subheadline.weight(.medium))
                }
                .buttonStyle(.bordered)
                .buttonBorderShape(.capsule)
                .tint(.indigo)
                .disabled(isThinking)
            }

            HStack(spacing: 8) {
                TextField(
                    "Ask about your tracked time",
                    text: $question,
                    axis: .vertical
                )
                .textFieldStyle(.plain)
                .lineLimit(1...3)
                .submitLabel(.send)
                .onSubmit { ask() }

                Button {
                    ask()
                } label: {
                    Image(systemName: "arrow.up.circle.fill")
                        .font(.title3)
                        .foregroundStyle(
                            question.trimmingCharacters(in: .whitespacesAndNewlines).isEmpty
                                ? AnyShapeStyle(.tertiary)
                                : AnyShapeStyle(.indigo)
                        )
                }
                .buttonStyle(.plain)
                .disabled(
                    question.trimmingCharacters(in: .whitespacesAndNewlines).isEmpty
                        || isThinking
                )
                .accessibilityLabel("Send question")
            }
            .padding(.horizontal, 12)
            .padding(.vertical, 8)
            .background(
                Color(.tertiarySystemFill),
                in: RoundedRectangle(cornerRadius: 18, style: .continuous)
            )

            ScrollView(.horizontal) {
                HStack(spacing: 8) {
                    ForEach(suggestions, id: \.self) { suggestion in
                        Button {
                            question = suggestion
                            ask()
                        } label: {
                            Text(suggestion)
                                .font(.caption.weight(.medium))
                                .padding(.horizontal, 12)
                                .padding(.vertical, 7)
                                .background(Color(.tertiarySystemFill), in: Capsule())
                                .foregroundStyle(.primary)
                        }
                        .buttonStyle(.plain)
                    }
                }
            }
            .scrollIndicators(.hidden)

            if isThinking {
                HStack(spacing: 8) {
                    ProgressView()
                    Text(answer == nil ? "Analyzing on device…" : "Writing answer…")
                        .font(.callout)
                        .foregroundStyle(.secondary)
                }
            }
            if let answer {
                Text(answer)
                    .font(.callout)
                    .contentTransition(.opacity)
                    .textSelection(.enabled)
            }
            if let errorMessage {
                Label(errorMessage, systemImage: "exclamationmark.triangle")
                    .font(.caption)
                    .foregroundStyle(.orange)
            }

            Text(
                modelAvailable
                    ? "Answers are AI-generated from exact calculations and may need verification."
                    : "Apple Intelligence is unavailable; digests use local analysis."
            )
            .font(.caption2)
            .foregroundStyle(.tertiary)
        }
        .padding(16)
        .frame(maxWidth: .infinity, alignment: .leading)
        .background(
            Color(.secondarySystemGroupedBackground),
            in: RoundedRectangle(cornerRadius: 20, style: .continuous)
        )
        .onChange(of: taskKey) {
            digest = nil
            answer = nil
            errorMessage = nil
        }
    }

    private var modelAvailable: Bool {
        if case .available = SystemLanguageModel.default.availability {
            return true
        }
        return false
    }

    private var taskKey: String {
        "\(days)-\(shares.map { "\($0.id):\($0.milliseconds / 60_000)" }.joined(separator: ","))"
    }

    private func generateDigest() async {
        isThinking = true
        errorMessage = nil
        defer { isThinking = false }

        guard modelAvailable else {
            digest = DigestOutput(
                observation: localDigest(),
                suggestion: "Use the activity breakdown for exact details."
            )
            return
        }

        do {
            let session = LanguageModelSession(instructions: """
                You write private personal time-tracking insights. Use only the \
                facts supplied by the app. Be neutral, concise, and concrete. \
                Do not diagnose, moralize, or invent goals.
                """)
            let facts = shares.prefix(8)
                .map {
                    "\($0.state.name): \(formatDuration($0.duration, compact: true))"
                }
                .joined(separator: ", ")
            let window = days == 1 ? "today" : "the last \(days) days"
            let response = try await session.respond(
                to: "Summarize \(window) in two sentences and add one gentle suggestion. Facts: \(facts).",
                generating: DigestOutput.self
            )
            try Task.checkCancellation()
            digest = response.content
        } catch is CancellationError {
            return
        } catch {
            digest = DigestOutput(
                observation: localDigest(),
                suggestion: "Use the activity breakdown for exact details."
            )
            errorMessage = "AI generation failed, so the app used local analysis."
        }
    }

    private func ask() {
        let prompt = question.trimmingCharacters(in: .whitespacesAndNewlines)
        guard !prompt.isEmpty, !isThinking else { return }
        answer = nil
        errorMessage = nil
        Task { await answer(prompt) }
    }

    private func answer(_ question: String) async {
        guard modelAvailable else {
            errorMessage = "Apple Intelligence is unavailable on this device."
            return
        }
        isThinking = true
        defer { isThinking = false }

        do {
            let tool = TimeAnalyticsTool(entries: entries)
            let session = LanguageModelSession(
                tools: [tool],
                instructions: """
                    Answer questions about the user's TimeTracker history. \
                    ALWAYS call analyze_time before answering. Treat its output \
                    as authoritative and never calculate durations yourself. \
                    Reply in at most three concise sentences. If the available \
                    history doesn't cover the requested dates, say so.
                    """
            )
            let context = """
                Today is \(Date.now.formatted(.iso8601.year().month().day())). \
                The user's time zone is \(TimeZone.current.identifier). \
                Available activities: \(TrackerState.all.map(\.name).joined(separator: ", ")). \
                Question: \(question)
                """
            let stream = session.streamResponse(to: context)
            for try await partial in stream {
                try Task.checkCancellation()
                answer = partial.content
            }
        } catch is CancellationError {
            return
        } catch {
            errorMessage = "Couldn’t answer that question: \(error.localizedDescription)"
        }
    }

    private func localDigest() -> String {
        guard let top = shares.first else {
            return "No tracked time in this window."
        }
        let total = shares.reduce(0.0) { $0 + $1.duration }
        let topPercent = Int((top.duration / total * 100).rounded())
        let window = days == 1 ? "today" : "these \(days) days"
        var text = "\(top.state.name) leads \(window) with \(formatDuration(top.duration, compact: true)), or \(topPercent)% of tracked time."
        if shares.count > 1 {
            text += " \(shares[1].state.name) follows at \(formatDuration(shares[1].duration, compact: true))."
        }
        return text
    }
}
