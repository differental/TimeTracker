import Foundation

nonisolated struct ActivityPredictor {
    struct Configuration: Sendable {
        var historyLengths = [1, 2, 4, 8, 16, 32]
        var baseTableSize = 512
        var taggedTableSize = 512
        var usefulnessAgingInterval = 256
        var maximumTrainingEntries = 10_000
    }

    struct BacktestResult: Sendable, Equatable {
        let evaluatedTransitions: Int
        let firstChoiceCorrect: Int
        let topThreeCorrect: Int
        let windowSize: Int

        var firstChoiceAccuracy: Double {
            guard evaluatedTransitions > 0 else { return 0 }
            return Double(firstChoiceCorrect) / Double(evaluatedTransitions)
        }

        var topThreeAccuracy: Double {
            guard evaluatedTransitions > 0 else { return 0 }
            return Double(topThreeCorrect) / Double(evaluatedTransitions)
        }
    }

    private struct Context {
        let weekday: Int
        let hour: Int
        let currentStateID: Int?
        let isTracking: Bool
        let history: [Int]
    }

    private struct Candidate {
        let stateID: Int
        var counter: UInt8
        var lastSeen: Int
    }

    private struct CandidateSet {
        var values: [Candidate] = []

        var ordered: [Candidate] {
            values.sorted {
                if $0.counter != $1.counter {
                    return $0.counter > $1.counter
                }
                if $0.lastSeen != $1.lastSeen {
                    return $0.lastSeen > $1.lastSeen
                }
                return $0.stateID < $1.stateID
            }
        }

        var topStateID: Int? { ordered.first?.stateID }

        var isConfident: Bool {
            let ranked = ordered
            guard let first = ranked.first else { return false }
            let second = ranked.dropFirst().first?.counter ?? 0
            return first.counter >= 4 && Int(first.counter) - Int(second) >= 2
        }

        mutating func observe(stateID: Int, sequence: Int) {
            if let index = values.firstIndex(where: { $0.stateID == stateID }) {
                values[index].counter = min(7, values[index].counter + 1)
                values[index].lastSeen = sequence
                return
            }

            if values.count < 3 {
                values.append(
                    Candidate(stateID: stateID, counter: 1, lastSeen: sequence)
                )
                return
            }

            for index in values.indices where values[index].counter > 0 {
                values[index].counter -= 1
            }
            values.removeAll { $0.counter == 0 }
            if values.count < 3 {
                values.append(
                    Candidate(stateID: stateID, counter: 1, lastSeen: sequence)
                )
            }
        }
    }

    private struct TaggedEntry {
        var tag: UInt16
        var candidates: CandidateSet
        var usefulness: UInt8
    }

    private struct Match {
        let table: Int
        let index: Int
        let entry: TaggedEntry
    }

    private let configuration: Configuration
    private let calendar: Calendar
    private var baseTable: [CandidateSet]
    private var taggedTables: [[TaggedEntry?]]
    private var globalCounts = Array(repeating: 0, count: TrackerState.all.count)
    private var recencyScores = Array(repeating: 0.0, count: TrackerState.all.count)
    private var globalLastSeen = Array(repeating: -1, count: TrackerState.all.count)
    private var recentHistory: [Int] = []
    private var trainingSequence = 0

    init(
        entries: [EntryRecord],
        calendar: Calendar = .current,
        configuration: Configuration = Configuration()
    ) {
        precondition(configuration.baseTableSize > 0)
        precondition(configuration.taggedTableSize > 0)
        precondition(configuration.usefulnessAgingInterval > 0)

        self.configuration = configuration
        self.calendar = calendar
        self.baseTable = Array(
            repeating: CandidateSet(),
            count: configuration.baseTableSize
        )
        self.taggedTables = configuration.historyLengths.map { _ in
            Array(repeating: nil, count: configuration.taggedTableSize)
        }

        let orderedEntries = entries
            .sorted {
                if $0.startTimestamp != $1.startTimestamp {
                    return $0.startTimestamp < $1.startTimestamp
                }
                return $0.index < $1.index
            }
            .suffix(configuration.maximumTrainingEntries)

        for entry in orderedEntries {
            train(targetStateID: entry.stateID, at: entry.startDate)
        }
    }

    static func backtest(
        entries: [EntryRecord],
        calendar: Calendar = .current,
        configuration: Configuration = Configuration(),
        windowSize: Int
    ) throws -> BacktestResult {
        precondition(windowSize > 0)
        let orderedEntries = entries.sorted {
            if $0.startTimestamp != $1.startTimestamp {
                return $0.startTimestamp < $1.startTimestamp
            }
            return $0.index < $1.index
        }
        guard orderedEntries.count > 1 else {
            return BacktestResult(
                evaluatedTransitions: 0,
                firstChoiceCorrect: 0,
                topThreeCorrect: 0,
                windowSize: windowSize
            )
        }

        var firstChoiceCorrect = 0
        var topThreeCorrect = 0
        var backtestConfiguration = configuration
        backtestConfiguration.maximumTrainingEntries = windowSize

        for targetIndex in 1..<orderedEntries.count {
            try Task.checkCancellation()
            let windowStart = max(0, targetIndex - windowSize)
            let trainingEntries = Array(
                orderedEntries[windowStart..<targetIndex]
            )
            let target = orderedEntries[targetIndex]
            let predictor = ActivityPredictor(
                entries: trainingEntries,
                calendar: calendar,
                configuration: backtestConfiguration
            )
            let predictions = predictor.predictions(
                at: target.startDate,
                currentStateID: orderedEntries[targetIndex - 1].stateID
            )
            if predictions.first?.id == target.stateID {
                firstChoiceCorrect += 1
            }
            if predictions.contains(where: { $0.id == target.stateID }) {
                topThreeCorrect += 1
            }
        }

        return BacktestResult(
            evaluatedTransitions: orderedEntries.count - 1,
            firstChoiceCorrect: firstChoiceCorrect,
            topThreeCorrect: topThreeCorrect,
            windowSize: windowSize
        )
    }

    func predictions(
        at date: Date = .now,
        currentStateID: Int?,
        limit: Int = 3
    ) -> [TrackerState] {
        guard limit > 0 else { return [] }
        let context = makeContext(
            date: date,
            currentStateID: currentStateID,
            history: recentHistory
        )
        let matches = matchingEntries(for: context)
        let base = baseTable[baseIndex(for: context)]
        let provider = matches.last
        let alternate = matches.dropLast().last

        var candidateSets: [CandidateSet] = []
        if let provider {
            if provider.entry.usefulness >= 2 || provider.entry.candidates.isConfident {
                candidateSets.append(provider.entry.candidates)
                if let alternate {
                    candidateSets.append(alternate.entry.candidates)
                }
            } else {
                if let alternate {
                    candidateSets.append(alternate.entry.candidates)
                }
                candidateSets.append(provider.entry.candidates)
            }
        }
        candidateSets.append(base)

        var stateIDs: [Int] = []
        func append(_ stateID: Int) {
            guard stateID != currentStateID, !stateIDs.contains(stateID) else {
                return
            }
            stateIDs.append(stateID)
        }

        for set in candidateSets {
            for candidate in set.ordered {
                append(candidate.stateID)
                if stateIDs.count == limit { break }
            }
            if stateIDs.count == limit { break }
        }

        if stateIDs.count < limit {
            let fallback = TrackerState.all.map(\.id).sorted {
                if recencyScores[$0] != recencyScores[$1] {
                    return recencyScores[$0] > recencyScores[$1]
                }
                if globalCounts[$0] != globalCounts[$1] {
                    return globalCounts[$0] > globalCounts[$1]
                }
                if globalLastSeen[$0] != globalLastSeen[$1] {
                    return globalLastSeen[$0] > globalLastSeen[$1]
                }
                return $0 < $1
            }
            for stateID in fallback {
                append(stateID)
                if stateIDs.count == limit { break }
            }
        }

        return stateIDs.map { TrackerState.all[$0] }
    }

    private mutating func train(targetStateID: Int, at date: Date) {
        let context = makeContext(
            date: date,
            currentStateID: recentHistory.last,
            history: recentHistory
        )
        let matches = matchingEntries(for: context)
        let provider = matches.last
        let alternateTop = matches.dropLast().last?.entry.candidates.topStateID
            ?? baseTable[baseIndex(for: context)].topStateID
        let providerTop = provider?.entry.candidates.topStateID
            ?? baseTable[baseIndex(for: context)].topStateID

        let base = baseIndex(for: context)
        baseTable[base].observe(
            stateID: targetStateID,
            sequence: trainingSequence
        )

        if let provider {
            var updated = provider.entry
            updated.candidates.observe(
                stateID: targetStateID,
                sequence: trainingSequence
            )
            if providerTop == targetStateID && alternateTop != targetStateID {
                updated.usefulness = min(3, updated.usefulness + 1)
            } else if providerTop != targetStateID && alternateTop == targetStateID {
                updated.usefulness = updated.usefulness > 0
                    ? updated.usefulness - 1
                    : 0
            }
            taggedTables[provider.table][provider.index] = updated
        }

        if providerTop != targetStateID {
            allocate(
                targetStateID: targetStateID,
                after: provider?.table ?? -1,
                context: context
            )
        }

        for index in recencyScores.indices {
            recencyScores[index] *= 0.95
        }
        globalCounts[targetStateID] += 1
        recencyScores[targetStateID] += 1
        globalLastSeen[targetStateID] = trainingSequence
        recentHistory.append(targetStateID)
        if recentHistory.count > (configuration.historyLengths.max() ?? 0) {
            recentHistory.removeFirst(
                recentHistory.count - (configuration.historyLengths.max() ?? 0)
            )
        }

        trainingSequence += 1
        if trainingSequence.isMultiple(of: configuration.usefulnessAgingInterval) {
            ageUsefulness()
        }
    }

    private mutating func allocate(
        targetStateID: Int,
        after providerTable: Int,
        context: Context
    ) {
        var allocations = 0
        for table in taggedTables.indices where table > providerTable {
            guard context.history.count >= configuration.historyLengths[table] else {
                continue
            }
            let location = taggedLocation(for: context, table: table)
            if var existing = taggedTables[table][location.index] {
                if existing.tag == location.tag {
                    continue
                }
                if existing.usefulness > 0 {
                    existing.usefulness -= 1
                    taggedTables[table][location.index] = existing
                    continue
                }
            }

            var candidates = CandidateSet()
            candidates.observe(
                stateID: targetStateID,
                sequence: trainingSequence
            )
            taggedTables[table][location.index] = TaggedEntry(
                tag: location.tag,
                candidates: candidates,
                usefulness: 0
            )
            allocations += 1
            if allocations == 2 { return }
        }
    }

    private mutating func ageUsefulness() {
        for table in taggedTables.indices {
            for index in taggedTables[table].indices {
                guard var entry = taggedTables[table][index] else { continue }
                entry.usefulness >>= 1
                taggedTables[table][index] = entry
            }
        }
    }

    private func makeContext(
        date: Date,
        currentStateID: Int?,
        history: [Int]
    ) -> Context {
        Context(
            weekday: calendar.component(.weekday, from: date),
            hour: calendar.component(.hour, from: date),
            currentStateID: currentStateID,
            isTracking: currentStateID != nil,
            history: history
        )
    }

    private func matchingEntries(for context: Context) -> [Match] {
        var result: [Match] = []
        for table in taggedTables.indices {
            guard context.history.count >= configuration.historyLengths[table] else {
                continue
            }
            let location = taggedLocation(for: context, table: table)
            guard let entry = taggedTables[table][location.index],
                  entry.tag == location.tag
            else { continue }
            result.append(Match(table: table, index: location.index, entry: entry))
        }
        return result
    }

    private func baseIndex(for context: Context) -> Int {
        Int(
            contextHash(context, historyLength: 0, salt: 0x243f_6a88)
                % UInt64(baseTable.count)
        )
    }

    private func taggedLocation(
        for context: Context,
        table: Int
    ) -> (index: Int, tag: UInt16) {
        let length = configuration.historyLengths[table]
        let indexHash = contextHash(
            context,
            historyLength: length,
            salt: UInt64(table + 1) &* 0x9e37_79b9
        )
        let tagHash = contextHash(
            context,
            historyLength: length,
            salt: UInt64(table + 1) &* 0x85eb_ca6b
        )
        return (
            Int(indexHash % UInt64(configuration.taggedTableSize)),
            UInt16(truncatingIfNeeded: tagHash ^ (tagHash >> 32))
        )
    }

    private func contextHash(
        _ context: Context,
        historyLength: Int,
        salt: UInt64
    ) -> UInt64 {
        var hash = 0xcbf2_9ce4_8422_2325 ^ salt
        hashValue(UInt64(context.weekday), into: &hash)
        hashValue(UInt64(context.hour), into: &hash)
        hashValue(UInt64((context.currentStateID ?? -1) + 1), into: &hash)
        hashValue(context.isTracking ? 1 : 0, into: &hash)
        if historyLength > 0 {
            for stateID in context.history.suffix(historyLength) {
                hashValue(UInt64(stateID + 1), into: &hash)
            }
        }
        return avalanche(hash)
    }

    private func hashValue(_ value: UInt64, into hash: inout UInt64) {
        hash ^= value &+ 0x9e37_79b9_7f4a_7c15
        hash &*= 0x0000_0100_0000_01b3
        hash ^= hash >> 32
    }

    private func avalanche(_ value: UInt64) -> UInt64 {
        var result = value
        result ^= result >> 30
        result &*= 0xbf58_476d_1ce4_e5b9
        result ^= result >> 27
        result &*= 0x94d0_49bb_1331_11eb
        result ^= result >> 31
        return result
    }
}
