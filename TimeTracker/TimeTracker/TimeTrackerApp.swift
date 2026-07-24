import CoreSpotlight
import AppIntents
import SwiftUI

@main
struct TimeTrackerApp: App {
    @State private var model = AppModel()

    var body: some Scene {
        WindowGroup {
            ContentView()
                .environment(model)
                .task {
                    try? await CSSearchableIndex.default().indexAppEntities(
                        TrackerState.all.map {
                            TrackerActivityEntity($0)
                        }
                    )
                }
        }
    }
}
