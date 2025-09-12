use axum::{
    Router, middleware,
    routing::{get, post, put},
};
use std::{env, net::SocketAddr};
use tokio::net::TcpListener;

mod auth;
use auth::auth_user;

mod constants;
use constants::AppState;

mod handlers;
use handlers::{
    add_entry, fetch_length, fetch_recent_states, fetch_summary_data, force_set_length, get_entry,
    serve_embedded_assets, update_entry,
};

mod pages;
use pages::{display_explanations, display_index, display_recents, display_summary};

mod utils;

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    dotenvy::dotenv()?;

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
        .layer(middleware::from_fn(auth_user));

    let public_app = Router::new().route("/static/{*file}", get(serve_embedded_assets));

    let app = Router::new()
        .merge(protected_app)
        .merge(public_app)
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
