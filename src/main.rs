use axum::{
    Router, middleware,
    routing::{get, post},
};
use std::{env, net::SocketAddr};
use tokio::net::TcpListener;

mod auth;
use auth::auth_user;

mod constants;
use constants::AppState;

mod handlers;
use handlers::{add_entry, fetch_data, fetch_length, force_set_length, serve_embedded_assets};

mod pages;
use pages::{display_explanations, display_index, display_summary};

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
        .route("/api/entry", post(add_entry))
        .route("/api/data", get(fetch_data))
        .route("/api/length", get(fetch_length))
        .route("/api/length", post(force_set_length))
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
