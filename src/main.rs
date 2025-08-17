use axum::{
    Router, middleware,
    routing::{get, post},
};
use std::net::SocketAddr;
use tokio::net::TcpListener;

mod auth;
use auth::auth_user;

mod constants;
use constants::AppState;

mod handlers;
use handlers::add_entry;

mod pages;
use pages::{display_index, display_summary};

mod utils;

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    let db = sled::open("./database.dat")?;
    let events = db.open_tree("events")?;
    let meta = db.open_tree("meta")?;

    let app_state = AppState { events, meta };

    let app = Router::new()
        .route("/", get(display_index))
        .route("/stats", get(display_summary))
        .route("/api/events", post(add_entry))
        .layer(middleware::from_fn(auth_user))
        .with_state(app_state);

    let addr = SocketAddr::from(([0, 0, 0, 0], 3000));
    println!("Server running on {addr}");

    let listener = TcpListener::bind(addr).await.unwrap();
    axum::serve(listener, app).await.unwrap();

    Ok(())
}
