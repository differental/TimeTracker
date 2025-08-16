use std::{
    env,
    net::SocketAddr,
    sync::{
        Arc, LazyLock,
        atomic::{AtomicU64, Ordering},
    },
};

use axum::{
    Router,
    extract::{Query, Request, State},
    http::StatusCode,
    middleware::{self, Next},
    response::{Html, IntoResponse, Response},
    routing::{get, post},
};
use chrono::{TimeZone, Utc};
use serde::Deserialize;
use sled::Tree;
use tera::{Context, Tera};
use tokio::net::TcpListener;

static ACCESS_KEY: LazyLock<String> = LazyLock::new(|| env::var("ACCESS_KEY").unwrap());

static TEMPLATES: LazyLock<Tera> = LazyLock::new(|| {
    let mut tera = match Tera::new("templates/*.html.tera") {
        Ok(t) => t,
        Err(e) => {
            println!("Parsing error(s): {}", e);
            ::std::process::exit(1);
        }
    };
    tera.autoescape_on(vec![".html"]);
    tera
});

#[derive(Clone)]
struct AppState {
    events: Tree,
    meta: Tree,
    len: Arc<AtomicU64>,
}

static STATES: [&'static str; 10] = [
    ("📚 Study"),
    ("💼 Work"),
    ("🚃 Commute"),
    ("🚣‍♂️ Sports"),
    ("📺 Entertainment"),
    ("📆 Appointment"),
    ("🥪 Maintenance"),
    ("🛏️ Sleep"),
    ("💬 Social"),
    ("🍹 Day Out"),
];

#[axum::debug_handler]
async fn display_index(State(state): State<AppState>) -> impl IntoResponse {
    let last_id = state.len.load(Ordering::SeqCst);

    if last_id == 0 {
        return (
            StatusCode::OK,
            Html("<p>Send a POST request to get started</p>"),
        )
            .into_response();
    }

    let bytes = state
        .events
        .get((last_id - 1).to_ne_bytes())
        .unwrap()
        .unwrap();
    let curr_state = u8::from_ne_bytes([bytes[0]]);
    let mut time_bytes = [0u8; 8];
    time_bytes.copy_from_slice(&bytes[1..]);
    let curr_starttime = i64::from_ne_bytes(time_bytes);

    let now = Utc::now();
    let starttime = Utc.timestamp_millis_opt(curr_starttime).unwrap();
    let duration = now - starttime;
    let (hours, minutes, seconds) = (
        duration.num_hours(),
        duration.num_minutes() - duration.num_hours() * 60,
        duration.num_seconds() - duration.num_minutes() * 60,
    );
    let elapsed_hms = format!("{:02}:{:02}:{:02}", hours, minutes, seconds);

    let mut ctx = Context::new();
    ctx.insert("current_state", STATES[curr_state as usize]);
    ctx.insert("elapsed_hms", &elapsed_hms);
    ctx.insert("key", &*ACCESS_KEY);
    ctx.insert("states", &STATES);
    ctx.insert("elapsed_ms", &duration.num_milliseconds());

    match TEMPLATES.render("templates/index.html.tera", &ctx) {
        Ok(str) => (StatusCode::OK, Html(str)).into_response(),
        Err(_) => (StatusCode::INTERNAL_SERVER_ERROR).into_response(),
    }
}

async fn display_summary(req: Request) -> Response {
    (StatusCode::OK).into_response()
}

async fn add_entry(req: Request) -> Response {
    (StatusCode::OK).into_response()
}

#[derive(Deserialize)]
struct AuthQueryParams {
    key: Option<String>,
}

async fn auth_user(
    Query(params): Query<AuthQueryParams>,
    request: Request,
    next: Next,
) -> impl IntoResponse {
    // Authentication layer, checks query param against key
    if let Some(val) = params.key {
        if val.trim() != *ACCESS_KEY {
            println!("{} {}", val.trim(), *ACCESS_KEY);
            return (StatusCode::FORBIDDEN).into_response();
        }
    } else {
        println!("no key found");
        return (StatusCode::FORBIDDEN).into_response();
    }

    next.run(request).await
}

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    let db = sled::open("./database.dat")?;
    let events = db.open_tree("events")?;
    let meta = db.open_tree("meta")?;

    println!("{}", *ACCESS_KEY);

    let initial_len = match meta.get(b"len")? {
        Some(v) => {
            let mut buffer = [0u8; 8];
            buffer.copy_from_slice(&v[..]);
            u64::from_ne_bytes(buffer)
        }
        None => 0u64,
    };

    let app_state = AppState {
        events,
        meta,
        len: Arc::new(AtomicU64::new(initial_len)),
    };

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
