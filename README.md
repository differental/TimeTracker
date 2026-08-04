# TimeTracker

Personal time tracker, served as a JSON API.

This was initially designed for personal use only, but it was fairly easy to expose most of the configuration options, so now you can also have your own time tracker and analyse your time!

The backend is designed with privacy in mind, and all requests without a valid `key` will result in a 403.

As of 2.0, this repo is the API only — it no longer serves HTML pages or static assets. The web UI lives in a separate client: [timetracker_client](https://github.com/differental/timetracker_client).

## Getting Started

Prerequisites: `cargo`

### Configuration

After cloning the repo, copy `.env.example` to `.env` and modify the contents:

- `ACCESS_KEY` is the required query parameter when visiting your site. **It must be present as `key` in all requests, anything else will receive a 403.**
- `DB_PATH` is the path to your `sled` database folder.
- `ADDR` is where your app will run. You should probably set it to `0.0.0.0:{PORT}` where `{PORT}` is a vacant port on your server.

Then, modify the "states" specified in `src/constants.rs`. You can have up to 64 different states, and you must specify an emoji (can be empty), a name, a description and a hex colour for each state. Clients read this list from `GET /api/states`, so they pick up your changes without needing their own copy.

### Deployment

You can compile & run your instance with `cargo run --release` and check it is up with `curl 'http://localhost:{PORT}/api/states?key={ACCESS_KEY}'`.

You can deploy your instance on a server and reach it through the server IP and port. You can also set up a reverse proxy and domain DNS records to reach it through a (sub)domain you own. To use the web UI, point the client's `TIMETRACKER_API_URL` at this server — the client holds the access key and proxies requests server-side, so the key never reaches the browser.

## API

Every route requires `?key={ACCESS_KEY}` and returns JSON.

| Method | Route | Purpose |
| --- | --- | --- |
| `GET` | `/api/states` | State metadata (emoji, name, description, colour), state count, emergency index, app version |
| `POST` | `/api/entry` | Log a state change |
| `GET` | `/api/entry/{idx}` | Read one entry |
| `PUT` | `/api/entry/{idx}` | Edit an entry's state and/or start time |
| `GET` | `/api/data` | Per-state totals over `days` |
| `GET` | `/api/length` | Number of entries |
| `POST` | `/api/length` | Force-set the entry count |
| `GET` | `/api/recents` | Recent `(state, start_timestamp)` pairs |
| `GET` | `/api/suggest` | Next-activity predictions |
| `GET` | `/api/export` | Full history as JSON |
| `POST` | `/api/import` | Replace history from JSON (32 MB limit) |

## Development

This project is in maintenance mode. There are no new features planned.

The project is still accepting issues or PRs. Thank you for your contribution.

## Why `sled`?

I was aiming to gain some experience with `sled` for another project. I am aware there are better solutions out there for this specific use case.
