import SwiftUI

struct HistoryView: View {
    @Environment(AppModel.self) private var model
    @State private var editingEntry: EntryRecord?

    var body: some View {
        NavigationStack {
            Group {
                if model.recents.isEmpty {
                    ContentUnavailableView(
                        "No entries yet",
                        systemImage: "clock.arrow.circlepath",
                        description: Text("Your activity log will appear here.")
                    )
                } else {
                    List {
                        ForEach(groupedByDay, id: \.day) { group in
                            Section(group.day.formatted(date: .abbreviated, time: .omitted)) {
                                ForEach(group.entries) { entry in
                                    row(entry)
                                }
                            }
                        }
                    }
                    .listStyle(.insetGrouped)
                }
            }
            .navigationTitle("History")
            .refreshable { await model.refresh() }
            .sheet(item: $editingEntry) { entry in
                EditEntrySheet(entry: entry)
                    .presentationDetents([.medium, .large])
            }
        }
    }

    private var groupedByDay: [(day: Date, entries: [EntryRecord])] {
        let calendar = Calendar.current
        let groups = Dictionary(grouping: model.recents) {
            calendar.startOfDay(for: $0.startDate)
        }
        return groups.keys.sorted(by: >).map { ($0, groups[$0]!) }
    }

    private func row(_ entry: EntryRecord) -> some View {
        Button {
            editingEntry = entry
        } label: {
            HStack(spacing: 12) {
                ZStack {
                    Circle()
                        .fill(entry.state.color.opacity(0.18))
                        .frame(width: 40, height: 40)
                    Image(systemName: entry.state.symbolName)
                        .symbolRenderingMode(.hierarchical)
                        .foregroundStyle(entry.state.color)
                }
                VStack(alignment: .leading, spacing: 2) {
                    Text(entry.state.name)
                        .font(.system(.body, design: .rounded, weight: .medium))
                        .foregroundStyle(.primary)
                    Text(
                        entry.startDate.formatted(date: .omitted, time: .shortened)
                        + (entry.endDate.map {
                            " – \($0.formatted(date: .omitted, time: .shortened))"
                        } ?? " – now")
                    )
                    .font(.caption)
                    .foregroundStyle(.secondary)
                }
                Spacer()
                if let duration = entry.duration {
                    Text(formatDuration(duration, compact: true))
                        .font(.system(.subheadline, design: .rounded, weight: .semibold))
                        .monospacedDigit()
                        .foregroundStyle(.secondary)
                } else {
                    Text("ongoing")
                        .font(.caption)
                        .fontWeight(.semibold)
                        .foregroundStyle(entry.state.color)
                        .padding(.horizontal, 8)
                        .padding(.vertical, 4)
                        .background(entry.state.color.opacity(0.15), in: Capsule())
                }
            }
        }
        .buttonStyle(.plain)
    }
}

struct EditEntrySheet: View {
    let entry: EntryRecord
    @Environment(AppModel.self) private var model
    @Environment(\.dismiss) private var dismiss

    @State private var stateID: Int
    @State private var startDate: Date
    @State private var isSaving = false
    @State private var errorMessage: String?

    init(entry: EntryRecord) {
        self.entry = entry
        _stateID = State(initialValue: entry.stateID)
        _startDate = State(initialValue: entry.startDate)
    }

    var body: some View {
        NavigationStack {
            Form {
                Section("Activity") {
                    Picker("State", selection: $stateID) {
                        ForEach(TrackerState.all) { state in
                            Label(state.name, systemImage: state.symbolName)
                                .tag(state.id)
                        }
                    }
                }
                Section("Start time") {
                    DatePicker(
                        "Started at",
                        selection: $startDate,
                        displayedComponents: [.date, .hourAndMinute]
                    )
                }
                if let errorMessage {
                    Section {
                        Text(errorMessage)
                            .font(.footnote)
                            .foregroundStyle(.red)
                    }
                }
            }
            .navigationTitle("Edit Entry")
            .navigationBarTitleDisplayMode(.inline)
            .toolbar {
                ToolbarItem(placement: .cancellationAction) {
                    Button("Cancel", role: .cancel) { dismiss() }
                }
                ToolbarItem(placement: .confirmationAction) {
                    Button("Save") { save() }
                        .disabled(isSaving)
                }
            }
        }
    }

    private func save() {
        isSaving = true
        errorMessage = nil
        Task {
            do {
                let sameStart = abs(startDate.timeIntervalSince(entry.startDate)) < 1
                try await model.updateEntry(
                    entry,
                    stateID: stateID == entry.stateID ? nil : stateID,
                    start: sameStart ? nil : startDate
                )
                dismiss()
            } catch {
                errorMessage = error.localizedDescription
                isSaving = false
            }
        }
    }
}

#Preview {
    HistoryView()
        .environment(AppModel())
}
