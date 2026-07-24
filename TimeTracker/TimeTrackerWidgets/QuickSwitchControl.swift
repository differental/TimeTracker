import AppIntents
import SwiftUI
import WidgetKit

struct QuickSwitchConfiguration: ControlConfigurationIntent {
    static let title: LocalizedStringResource = "Quick Switch"
    static let description = IntentDescription(
        "Choose which activity this control starts."
    )

    @Parameter(title: "Activity")
    var activity: TrackerActivityEntity?

    init() {
        activity = TrackerActivityEntity(id: 1)
    }
}

struct QuickSwitchControl: ControlWidget {
    var body: some ControlWidgetConfiguration {
        AppIntentControlConfiguration(
            kind: "at.janez.TimeTracker.quickswitch",
            intent: QuickSwitchConfiguration.self
        ) { configuration in
            let activity = configuration.activity ?? TrackerActivityEntity(id: 1)
            let state = activity.state
            ControlWidgetButton(
                action: SwitchActivityIntent(activity: activity)
            ) {
                Label("Track \(state.name)", systemImage: state.symbolName)
            }
        }
        .displayName("Quick Switch")
        .description("Start tracking an activity with one tap.")
    }
}
