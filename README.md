# TimeTracker

Personal time tracker with a simple UI. 

This was initially designed for personal use only, but it was fairly easy to expose most of the configuration options, so now you can also have your own time tracker and analyse your time!

The backend is designed with privacy in mind. Every request authenticates with
the `?key=<ACCESS_KEY>` URL parameter, used by both the web UI and the native
iOS client.

## Getting Started

Prerequisites: `cargo`

### Configuration

After cloning the repo, copy `.env.example` to `.env` and modify the contents:

- `ACCESS_KEY` authenticates every request, passed as `?key=` in the browser and
  by API clients.
- `DB_PATH` is the path to your `sled` database folder.
- `ADDR` is where your app will run. You should probably set it to `0.0.0.0:{PORT}` where `{PORT}` is a vacant port on your server.

Then, modify the "states" specified in `src/constants.rs`. You can have up to 64 different states, and you must specify an emoji (can be empty), a name, a description (displayed in "Explanations") and a hex colour (for the pie chart) for each state.

### Deployment

You can compile & run your instance with `cargo run --release` and access it through `http://localhost:{PORT}/?key={ACCESS_KEY}`.

You can deploy your instance on a server and access it through the server IP and port. You can also set up a reverse proxy and domain DNS records to access it through a (sub)domain you own.

## Development

This project is in maintenance mode. There are no new features planned.

The project is still accepting issues or PRs. Thank you for your contribution.

## Native iOS app

The `TimeTracker` folder contains the iOS 26 SwiftUI client and widget extension.
Open `TimeTracker/TimeTracker.xcodeproj` in Xcode 26 and run the `TimeTracker`
scheme. On first launch, enter the server base URL and `ACCESS_KEY` in Settings.

Platform integrations include:

- Home Screen, Lock Screen, StandBy, and paired Apple Watch widgets with
  interactive activity switching, relevance hints, and a shared offline snapshot
- a configurable Control Center quick switch
- App Intents backed by searchable `AppEntity` activities, Spotlight indexing,
  Shortcuts phrases, a current-activity snippet, and Focus filters
- bounded focus sessions using AlarmKit, including Lock Screen, Dynamic Island,
  StandBy, and Apple Watch controls
- private, on-device Foundation Models digests and questions; exact duration
  calculations are supplied by a local tool and AI output is clearly labeled
- optional on-device Calendar suggestions
- reduced-motion, Low Power Mode, Dynamic Type, chart, and VoiceOver support

The app stores the access key in a shared Keychain access group and uses an app
group snapshot so widgets do not depend on a successful network request.

### Apple capabilities

For both the app and widget extension, select your development team and enable:

- App Groups: `group.at.janez.TimeTracker`
- Keychain Sharing: `$(AppIdentifierPrefix)at.janez.TimeTracker.shared`
- Live Activities

Push Notifications are **not** required. The app assumes the phone is the only
client that changes state, so widgets and Live Activities update in-process:
Live Activities through a local `Activity.update`, widgets through their refresh
timeline plus an immediate reload after every change made on the device. No APNs
key, no push entitlement, and no paid Apple Developer Program team is needed —
the app signs with a Personal Team.

AlarmKit and Calendar usage descriptions are already present in the app's
`Info.plist`; users are prompted only when they choose those features.

### Server compatibility

The iOS client talks to an unmodified TimeTracker server and needs no
server-side changes. Any existing deployment works as-is — enter its base URL
and `ACCESS_KEY` in Settings.

If you edit state from somewhere other than the phone (the web UI on another
device, or the API directly), the app will not learn about it until its next
refresh. Making that instant would require server-driven APNs push, which this
build deliberately does not include.

### Verification

```sh
cargo fmt --check
cargo test
xcodebuild -project TimeTracker/TimeTracker.xcodeproj \
  -scheme TimeTracker \
  -destination 'platform=iOS Simulator,name=iPhone 17 Pro' test
```

The UI tests use a real disposable backend. Set `TT_UITEST_SERVER_URL` and
`TT_UITEST_ACCESS_KEY` to enable the switch-flow and accessibility-audit tests.
The predictor backtest skips unless its lifelog fixture is present locally.

## Why `sled`?

I was aiming to gain some experience with `sled` for another project. I am aware there are better solutions out there for this specific use case.
