# TimeTracker

Personal time tracker with a simple UI. 

This was initially designed for personal use only, but it was fairly easy to expose most of the configuration options, so now you can also have your own time tracker and analyse your time!

The backend is designed with privacy in mind, and all requests without a valid `key` will result in a 403.

## Getting Started

Prerequisites: `cargo`

### Configuration

After cloning the repo, copy `.env.example` to `.env` and modify the contents:

- `ACCESS_KEY` is the required query parameter when visiting your site. **It must be present as `key` in all requests, anything else will receive a 403.**
- `DB_PATH` is the path to your `sled` database folder.
- `ADDR` is where your app will run. You should probably set it to `0.0.0.0:{PORT}` where `{PORT}` is a vacant port on your server.

Then, modify the "states" specified in `src/constants.rs`. You can have up to 64 different states, and you must specify an emoji (can be empty), a name, a description (displayed in "Explanations") and a hex colour (for the pie chart) for each state.

### Deployment

You can compile & run your instance with `cargo run --release` and access it through `http://localhost:{PORT}/?key={ACCESS_KEY}`.

You can deploy your instance on a server and access it through the server IP and port. You can also set up a reverse proxy and domain DNS records to access it through a (sub)domain you own.

## Development

This project is in maintenance mode. There are no new features planned.

The project is still accepting issues or PRs. Thank you for your contribution.

## Why `sled`?

I was aiming to gain some experience with `sled` for another project. I am aware there are better solutions out there for this specific use case.
