import SwiftUI

struct ContentView: View {
    @Environment(AppModel.self) private var model
    @Environment(\.scenePhase) private var scenePhase
    @State private var showSettings = false
    @AppStorage("selectedTab") private var selectedTab = 0

    init() {
        #if DEBUG
        if let tab = ProcessInfo.processInfo.environment["TT_TAB"].flatMap(Int.init) {
            UserDefaults.standard.set(tab, forKey: "selectedTab")
        }
        #endif
    }

    var body: some View {
        TabView(selection: $selectedTab) {
            Tab("Now", systemImage: "timer", value: 0) {
                NowView()
            }
            Tab("Insights", systemImage: "chart.pie.fill", value: 1) {
                InsightsView()
            }
            Tab("History", systemImage: "clock.arrow.circlepath", value: 2) {
                HistoryView()
            }
        }
        .task {
            if model.isConfigured {
                await model.refresh()
            } else {
                showSettings = true
            }
        }
        .onChange(of: scenePhase) { _, phase in
            guard phase == .active, model.isConfigured else { return }
            Task { await model.refresh() }
        }
        .onOpenURL { url in
            guard url.scheme == "timetracker" else { return }
            switch url.host {
            case "insights":
                selectedTab = 1
            case "history":
                selectedTab = 2
            default:
                selectedTab = 0
            }
        }
        .sheet(isPresented: $showSettings) {
            SettingsView()
        }
    }
}

#Preview {
    ContentView()
        .environment(AppModel())
}
