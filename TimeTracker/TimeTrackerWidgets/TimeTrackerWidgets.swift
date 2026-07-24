import SwiftUI
import WidgetKit

@main
struct TimeTrackerWidgetsBundle: WidgetBundle {
    var body: some Widget {
        CurrentActivityWidget()
        QuickSwitchControl()
        TrackerLiveActivity()
        FocusAlarmLiveActivity()
    }
}
