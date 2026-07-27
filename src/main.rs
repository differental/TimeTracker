// TimeTracker - Rust-based web app that tracks and analyses user's daily routine to provide insight in time management.
// Copyright (C) 2025 Brian Chen (differental)
//
// This program is free software: you can redistribute it and/or modify
// it under the terms of the GNU Affero General Public License as published
// by the Free Software Foundation, version 3.
//
// This program is distributed in the hope that it will be useful,
// but WITHOUT ANY WARRANTY; without even the implied warranty of
// MERCHANTABILITY or FITNESS FOR A PARTICULAR PURPOSE.  See the
// GNU Affero General Public License for more details.
//
// You should have received a copy of the GNU Affero General Public License
// along with this program. If not, see <https://www.gnu.org/licenses/>.

use axum::{
    Router,
    extract::DefaultBodyLimit,
    middleware,
    routing::{get, post, put},
};
use std::{env, net::SocketAddr};
use tokio::net::TcpListener;
use tower_http::compression::{
    CompressionLayer, DefaultPredicate, Predicate, predicate::NotForContentType,
};

mod auth;
use auth::auth_user;

mod constants;
use constants::AppState;

mod handlers;
use handlers::{
    add_entry, export_data, fetch_length, fetch_recent_states, fetch_summary_data,
    force_set_length, get_entry, import_data, serve_embedded_assets, suggest_next_states,
    update_entry,
};

mod predictor;

mod pages;
use pages::{display_explanations, display_index, display_recents, display_summary};

mod utils;

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    // Allow .env to not exist and environment variables to be passed directly, for example in Docker
    dotenvy::dotenv().ok();

    let db = sled::open(env::var("DB_PATH").unwrap())?;
    let events = db.open_tree("events")?;
    let meta = db.open_tree("meta")?;

    let app_state = AppState { events, meta };

    let protected_app = Router::new()
        .route("/", get(display_index))
        .route("/summary", get(display_summary))
        .route("/explanations", get(display_explanations))
        .route("/recents", get(display_recents))
        .route("/api/entry", post(add_entry))
        .route("/api/entry/{entry_idx}", get(get_entry))
        .route("/api/entry/{entry_idx}", put(update_entry))
        .route("/api/data", get(fetch_summary_data))
        .route("/api/length", get(fetch_length))
        .route("/api/length", post(force_set_length))
        .route("/api/recents", get(fetch_recent_states))
        .route("/api/suggest", get(suggest_next_states))
        .route("/api/export", get(export_data))
        .route(
            "/api/import",
            post(import_data).layer(DefaultBodyLimit::max(32 * 1024 * 1024)),
        )
        .layer(middleware::from_fn(auth_user));

    let public_app = Router::new().route("/static/{*file}", get(serve_embedded_assets));

    let compression = CompressionLayer::new()
        .compress_when(DefaultPredicate::new().and(NotForContentType::const_new("font/")));

    let app = Router::new()
        .merge(protected_app)
        .merge(public_app)
        .layer(compression)
        .with_state(app_state);

    let addr = env::var("ADDR")
        .unwrap()
        .parse::<SocketAddr>()
        .expect("Wrong address format.");
    let listener = TcpListener::bind(addr).await.unwrap();

    println!("Server running on {addr}");

    axum::serve(listener, app).await.unwrap();

    Ok(())
}
