import Foundation
import Testing
@testable import TimeTracker

struct TimeTrackerTests {
    @Test func durationFormatting() {
        #expect(formatDuration(3_900) == "01:05:00")
        #expect(formatDuration(3_900, compact: true) == "1h 5m")
        #expect(formatDuration(420, compact: true) == "7m")
        #expect(formatDuration(-5) == "00:00:00")
    }

    @Test func stateMappingMatchesServerContract() async throws {
        let ids = Array(0...14)
        #expect(TrackerState.all.map(\.id) == ids)
        let entities = try await TrackerActivityQuery().allEntities()
        #expect(entities.map(\.id) == ids)
        #expect(entities.map(\.state.name) == TrackerState.all.map(\.name))
    }

    @Test func entryDurationUsesFollowingStartTime() {
        let entry = EntryRecord(
            index: 4,
            stateID: 3,
            startTimestamp: 1_000,
            endTimestamp: 61_000
        )
        #expect(entry.duration == 60)
        #expect(entry.state.name == "Projects")
    }

    @Test func predictorColdStartUsesCanonicalOrder() {
        let predictor = ActivityPredictor(entries: [])
        let predictions = predictor.predictions(
            at: Date(timeIntervalSince1970: 1_700_000_000),
            currentStateID: nil
        )

        #expect(predictions.map(\.id) == [0, 1, 2])
    }

    @Test func predictorAlwaysReturnsThreeUniqueNonCurrentActivities() {
        let entries = makeEntries(states: [0, 1, 0, 2, 0, 1, 0, 3])
        let predictor = ActivityPredictor(entries: entries)
        let predictions = predictor.predictions(
            at: entries.last!.startDate.addingTimeInterval(60),
            currentStateID: 3
        )

        #expect(predictions.count == 3)
        #expect(Set(predictions.map(\.id)).count == 3)
        #expect(!predictions.map(\.id).contains(3))
    }

    @Test func predictorLearnsRepeatingActionSequence() {
        let states = Array(repeating: [0, 1, 2], count: 20).flatMap { $0 }
        let entries = makeEntries(states: states)
        let predictor = ActivityPredictor(
            entries: entries,
            configuration: .init(
                historyLengths: [1, 2, 4, 8],
                baseTableSize: 1_024,
                taggedTableSize: 1_024,
                usefulnessAgingInterval: 256,
                maximumTrainingEntries: 10_000
            )
        )
        let predictions = predictor.predictions(
            at: entries.last!.startDate.addingTimeInterval(60),
            currentStateID: 2
        )

        #expect(predictions.first?.id == 0)
    }

    @Test func predictorHonorsTrainingEntryLimit() {
        let entries = makeEntries(states: Array(repeating: 1, count: 20) + [2, 2, 2])
        let predictor = ActivityPredictor(
            entries: entries,
            configuration: .init(
                historyLengths: [1, 2],
                baseTableSize: 128,
                taggedTableSize: 128,
                usefulnessAgingInterval: 256,
                maximumTrainingEntries: 3
            )
        )
        let predictions = predictor.predictions(
            at: entries.last!.startDate.addingTimeInterval(60),
            currentStateID: nil
        )

        #expect(predictions.first?.id == 2)
    }

    @Test func predictorUsesWeekdayAndHourContext() {
        var calendar = Calendar(identifier: .gregorian)
        calendar.timeZone = TimeZone(secondsFromGMT: 0)!
        var entries: [EntryRecord] = []

        for week in 0..<8 {
            let day = 5 + week * 7
            let observations = [
                (hour: 8, minute: 59, stateID: 0),
                (hour: 9, minute: 0, stateID: 1),
                (hour: 17, minute: 59, stateID: 0),
                (hour: 18, minute: 0, stateID: 2),
            ]
            for observation in observations {
                let date = calendar.date(
                    from: DateComponents(
                        year: 2026,
                        month: 1,
                        day: day,
                        hour: observation.hour,
                        minute: observation.minute
                    )
                )!
                entries.append(
                    EntryRecord(
                        index: UInt64(entries.count),
                        stateID: observation.stateID,
                        startTimestamp: Int64(date.timeIntervalSince1970 * 1_000),
                        endTimestamp: nil
                    )
                )
            }
        }

        let predictor = ActivityPredictor(
            entries: entries,
            calendar: calendar,
            configuration: .init(
                historyLengths: [1, 2, 4],
                baseTableSize: 4_096,
                taggedTableSize: 1_024,
                usefulnessAgingInterval: 256,
                maximumTrainingEntries: 10_000
            )
        )
        let morning = calendar.date(
            from: DateComponents(year: 2026, month: 3, day: 2, hour: 9)
        )!
        let evening = calendar.date(
            from: DateComponents(year: 2026, month: 3, day: 2, hour: 18)
        )!

        #expect(
            predictor.predictions(at: morning, currentStateID: 0).first?.id == 1
        )
        #expect(
            predictor.predictions(at: evening, currentStateID: 0).first?.id == 2
        )
    }

    @Test func predictorBacktestReplaysEveryTransitionWithoutFutureData() throws {
        let states = Array(repeating: [0, 1, 2], count: 20).flatMap { $0 }
        let result = try ActivityPredictor.backtest(
            entries: makeEntries(states: states),
            configuration: .init(
                historyLengths: [1, 2, 4],
                baseTableSize: 256,
                taggedTableSize: 256,
                usefulnessAgingInterval: 128,
                maximumTrainingEntries: 10_000
            ),
            windowSize: 25
        )

        #expect(result.evaluatedTransitions == states.count - 1)
        #expect(result.firstChoiceCorrect > result.evaluatedTransitions / 2)
        #expect(result.topThreeCorrect >= result.firstChoiceCorrect)
        #expect(result.windowSize == 25)
    }

    private func makeEntries(states: [Int]) -> [EntryRecord] {
        let start = Int64(1_700_000_000_000)
        return states.enumerated().map { offset, stateID in
            EntryRecord(
                index: UInt64(offset),
                stateID: stateID,
                startTimestamp: start + Int64(offset * 60_000),
                endTimestamp: nil
            )
        }
    }
}
