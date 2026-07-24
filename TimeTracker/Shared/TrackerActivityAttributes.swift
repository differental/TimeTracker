import ActivityKit
import Foundation
import OSLog

struct TrackerActivityAttributes: ActivityAttributes {
    struct ContentState: Codable, Hashable {
        var stateID: Int
        var startTimestamp: Int64
        var expectedEndTimestamp: Int64?

        var start: Date {
            Date(timeIntervalSince1970: Double(startTimestamp) / 1_000)
        }

        var expectedEnd: Date? {
            expectedEndTimestamp.map {
                Date(timeIntervalSince1970: Double($0) / 1_000)
            }
        }
    }
}

@MainActor
enum LiveActivityController {
    private static let logger = Logger(
        subsystem: "at.janez.TimeTracker",
        category: "LiveActivity"
    )

    static func reconcile(
        stateID: Int?,
        startTimestamp: Int64?,
        expectedEndTimestamp: Int64? = nil,
        createIfNeeded: Bool
    ) async {
        guard let stateID, let startTimestamp else {
            await endAll()
            return
        }

        let maximumEnd = startTimestamp + (8 * 60 * 60 * 1_000)
        let effectiveEnd = min(expectedEndTimestamp ?? maximumEnd, maximumEnd)
        let content = ActivityContent(
            state: TrackerActivityAttributes.ContentState(
                stateID: stateID,
                startTimestamp: startTimestamp,
                expectedEndTimestamp: expectedEndTimestamp
            ),
            staleDate: Date(
                timeIntervalSince1970: Double(effectiveEnd) / 1_000
            )
        )
        let activities = Activity<TrackerActivityAttributes>.activities

        if let primary = activities.first {
            await primary.update(content)
            observePushTokens(for: primary)
            for duplicate in activities.dropFirst() {
                await duplicate.end(nil, dismissalPolicy: .immediate)
            }
            return
        }

        guard createIfNeeded,
              ActivityAuthorizationInfo().areActivitiesEnabled
        else { return }

        do {
            _ = try Activity.request(
                attributes: TrackerActivityAttributes(),
                content: content,
                pushType: .token
            )
            if let activity = Activity<TrackerActivityAttributes>.activities.first {
                observePushTokens(for: activity)
            }
        } catch {
            logger.error(
                "Live Activity request failed: \(error.localizedDescription, privacy: .public)"
            )
        }
    }

    static func endAll() async {
        for activity in Activity<TrackerActivityAttributes>.activities {
            await activity.end(nil, dismissalPolicy: .immediate)
        }
    }

    private static var observedActivityIDs = Set<String>()

    private static func observePushTokens(
        for activity: Activity<TrackerActivityAttributes>
    ) {
        guard observedActivityIDs.insert(activity.id).inserted else { return }
        Task {
            for await token in activity.pushTokenUpdates {
                guard let config = ServerConfig.load() else { continue }
                do {
                    try await APIClient(config: config).registerLiveActivityPushToken(
                        token,
                        activityID: activity.id,
                        topic: "at.janez.TimeTracker.push-type.liveactivity",
                        environment: pushEnvironment
                    )
                } catch {
                    logger.error(
                        "Live Activity push registration failed: \(error.localizedDescription, privacy: .public)"
                    )
                }
            }
        }
    }

    private static var pushEnvironment: String {
        #if DEBUG
        "sandbox"
        #else
        "production"
        #endif
    }
}
