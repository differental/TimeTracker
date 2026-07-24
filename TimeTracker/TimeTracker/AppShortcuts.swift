import AppIntents

struct TrackerShortcuts: AppShortcutsProvider {
    static var appShortcuts: [AppShortcut] {
        AppShortcut(
            intent: SwitchActivityIntent(),
            phrases: [
                "Switch activity in \(.applicationName)",
                "Start tracking in \(.applicationName)",
            ],
            shortTitle: "Switch Activity",
            systemImageName: "timer"
        )
        AppShortcut(
            intent: CurrentActivityIntent(),
            phrases: [
                "What am I doing in \(.applicationName)",
                "Current activity in \(.applicationName)",
            ],
            shortTitle: "Current Activity",
            systemImageName: "questionmark.circle"
        )
        AppShortcut(
            intent: StartFocusSessionIntent(),
            phrases: [
                "Start a focus session in \(.applicationName)",
                "Start a timed activity in \(.applicationName)",
            ],
            shortTitle: "Focus Session",
            systemImageName: "timer.circle"
        )
    }
}
