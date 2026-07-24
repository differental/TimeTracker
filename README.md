# TimeTracker

Personal time tracker with a simple UI. 

This was initially designed for personal use only, but it was fairly easy to expose most of the configuration options, so now you can also have your own time tracker and analyse your time!

The backend is designed with privacy in mind. API clients should send
`Authorization: Bearer <ACCESS_KEY>`; the existing `?key=` URL parameter remains
supported for the web UI and older clients.

## Getting Started

Prerequisites: `cargo`

### Configuration

After cloning the repo, copy `.env.example` to `.env` and modify the contents:

- `ACCESS_KEY` authenticates every request. Use it as `?key=` in the browser or
  as a Bearer token from an API client.
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
  interactive activity switching, relevance hints, a shared offline snapshot,
  and optional WidgetKit push updates
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
- Push Notifications
- Live Activities

The included entitlements select APNs development or production through the
`APS_ENVIRONMENT` build setting. AlarmKit and Calendar usage descriptions are
already present in the app's `Info.plist`; users are prompted only when they
choose those features.

### Optional APNs updates

Widget and Live Activity updates work locally without APNs. To let the backend
refresh them immediately after a state change, create an Apple Push Notification
authentication key and set:

```dotenv
APNS_KEY_PATH=/absolute/path/to/AuthKey_XXXXXXXXXX.p8
APNS_KEY_ID=XXXXXXXXXX
APNS_TEAM_ID=XXXXXXXXXX
APNS_TOPIC_PREFIX=at.janez.TimeTracker
```

Mount the `.p8` file as a deployment secret and never commit it. If these
variables are absent, the server safely skips remote push delivery. Device
tokens are registered through authenticated endpoints and removed when APNs
reports that they are no longer valid.

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

## Why `sled`?

I was aiming to gain some experience with `sled` for another project. I am aware there are better solutions out there for this specific use case.
