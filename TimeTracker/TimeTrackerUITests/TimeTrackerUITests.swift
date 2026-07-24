import XCTest

final class TimeTrackerUITests: XCTestCase {
    override func setUpWithError() throws {
        continueAfterFailure = false
    }

    @MainActor
    private func launchConfigured() throws -> XCUIApplication {
        let environment = ProcessInfo.processInfo.environment
        guard let serverURL = environment["TT_UITEST_SERVER_URL"],
              let accessKey = environment["TT_UITEST_ACCESS_KEY"],
              !serverURL.isEmpty,
              !accessKey.isEmpty
        else {
            throw XCTSkip(
                "Set TT_UITEST_SERVER_URL and TT_UITEST_ACCESS_KEY to run the server-backed UI test."
            )
        }
        let app = XCUIApplication()
        app.launchEnvironment["TT_SERVER_URL"] = serverURL
        app.launchEnvironment["TT_ACCESS_KEY"] = accessKey
        app.launch()
        return app
    }

    @MainActor
    func testCoreSwitchFlow() throws {
        let app = try launchConfigured()
        let target = app.buttons["predictedActivity.0"]
        XCTAssertTrue(target.waitForExistence(timeout: 15), "Predictions did not load")
        XCTAssertTrue(app.buttons["predictedActivity.1"].exists)
        XCTAssertTrue(app.buttons["predictedActivity.2"].exists)
        let targetName = target.label.replacingOccurrences(
            of: "Switch to ",
            with: ""
        )
        target.tap()

        XCTAssertFalse(app.buttons["Switch"].exists, "Switch should be immediate")

        let hero = app.staticTexts["currentActivityName"]
        let heroUpdated = XCTNSPredicateExpectation(
            predicate: NSPredicate(format: "label == %@", targetName),
            object: hero
        )
        XCTAssertEqual(
            XCTWaiter.wait(for: [heroUpdated], timeout: 15),
            .completed,
            "Now view did not update to \(targetName)"
        )

        let more = app.buttons["moreActivities"]
        if !more.isHittable {
            app.swipeUp()
        }
        XCTAssertTrue(more.waitForExistence(timeout: 5))
        more.tap()
        XCTAssertTrue(
            app.navigationBars["All Activities"].waitForExistence(timeout: 5)
        )

        let work = app.buttons["moreActivity.1"]
        let study = app.buttons["moreActivity.0"]
        let (secondTarget, secondTargetName) = work.isEnabled
            ? (work, "Work")
            : (study, "Study")
        secondTarget.tap()

        let secondHeroUpdated = XCTNSPredicateExpectation(
            predicate: NSPredicate(format: "label == %@", secondTargetName),
            object: hero
        )
        XCTAssertEqual(
            XCTWaiter.wait(for: [secondHeroUpdated], timeout: 15),
            .completed,
            "More did not switch to \(secondTargetName)"
        )

        app.tabBars.buttons["Insights"].tap()
        XCTAssertTrue(
            app.navigationBars["Insights"].waitForExistence(timeout: 5),
            "Insights screen did not load"
        )

        app.tabBars.buttons["History"].tap()
        XCTAssertTrue(
            app.navigationBars["History"].waitForExistence(timeout: 5),
            "History screen did not load"
        )
        XCTAssertTrue(
            app.buttons.matching(
                NSPredicate(format: "label CONTAINS %@", secondTargetName)
            ).firstMatch.exists,
            "History entries were not rendered"
        )

    }

    @MainActor
    func testAccessibilityAudit() throws {
        let app = try launchConfigured()
        XCTAssertTrue(
            app.buttons["predictedActivity.0"].waitForExistence(timeout: 15),
            "Now screen did not finish loading"
        )
        try app.performAccessibilityAudit()

        app.tabBars.buttons["Insights"].tap()
        XCTAssertTrue(app.navigationBars["Insights"].waitForExistence(timeout: 5))
        try app.performAccessibilityAudit()

        app.tabBars.buttons["History"].tap()
        XCTAssertTrue(app.navigationBars["History"].waitForExistence(timeout: 5))
        try app.performAccessibilityAudit()
    }
}
