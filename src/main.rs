use std::{
    env,
    net::SocketAddr,
    sync::{
        Arc, LazyLock,
        atomic::{AtomicU64, Ordering},
    },
};

use axum::{
    debug_handler, extract::{Query, Request, State}, http::StatusCode, middleware::{self, Next}, response::{Html, IntoResponse, Response}, routing::{get, post}, Json, Router
};
use chrono::{TimeZone, Utc};
use num::{pow, traits::ToBytes};
use serde::Deserialize;
use sled::{IVec, Tree};
use askama::Template;
use tokio::net::TcpListener;

static ACCESS_KEY: LazyLock<String> = LazyLock::new(|| env::var("ACCESS_KEY").unwrap());

#[derive(Template)]
#[template(path = "index.html")]
struct IndexPageTemplate<'a> {
    key: &'a str,
    states: [&'a str; 10],
    current_state: &'a str,
    elapsed_hms: String,
    elapsed_ms: i64,
}

#[derive(Clone)]
struct AppState {
    events: Tree,
    meta: Tree
}

static STATES: [&str; 10] = [
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

fn ivec_to_u64(v: IVec) -> u64 {
    let slice = v.as_ref();
    let mut bytes = [0u8; 8];
    bytes.copy_from_slice(&slice[0..8]);
    u64::from_ne_bytes(bytes)
}

fn to_ivec<T: ToBytes>(n: T) -> IVec where IVec: for<'a> From<&'a T::Bytes> {
    // There's gotta be some way to not express this in such an ugly way...
    let bytes = n.to_ne_bytes();
    IVec::from(&bytes)
}

fn incr_length(meta: &mut Tree) -> u64 {
    // Inserts 0 if doesn't exist, returns new length
    let len = match meta.get(b"len").unwrap() {
        Some(val) => ivec_to_u64(val),
        None => 0
    };

    let v = to_ivec((len + 1) as u64);

    meta.insert(b"len", v);

    len + 1
}

fn get_length(meta: &Tree) -> u64 {
    match meta.get(b"len").unwrap() {
        Some(val) => ivec_to_u64(val),
        None => {
            meta.insert(b"len", to_ivec(0u64));
            0
        }
    }
}

async fn display_index(State(state): State<AppState>) -> impl IntoResponse {
    let last_id = get_length(&state.meta);


    println!("{}", last_id);



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

    let page = IndexPageTemplate {
        key: &*ACCESS_KEY,
        states: STATES,
        current_state: STATES[curr_state as usize],
        elapsed_hms,
        elapsed_ms: duration.num_milliseconds(),
    };

    let rendered = page.render().unwrap();
    (StatusCode::OK, Html(rendered)).into_response()
}

async fn display_summary(req: Request) -> Response {
    (StatusCode::OK).into_response()
}

#[derive(Deserialize)]
struct AddEntryRequest {
    new_state: u8, // 0-indexed state
    start_timestamp: i64,
}


async fn add_entry(
    State(mut state): State<AppState>,
    Json(payload): Json<AddEntryRequest>,
) -> Response {
    let AddEntryRequest {
        new_state,
        start_timestamp,
    } = payload;

    // Add timestamp etc. checks here! Remember timestamps should be UTC

    let new_key = incr_length(&mut state.meta) - 1;

    // Inserted element: First byte is new_state, next 8 bytes are start_timestamp
    let mut bytes = [0u8; 9];
    bytes[0] = new_state;
    bytes[1..].copy_from_slice(&start_timestamp.to_ne_bytes());

    match state.events.insert(to_ivec(new_key), IVec::from(&bytes)) {
        Ok(_) => (),
        Err(err) => {
            println!("{err:?}");
            return (StatusCode::INTERNAL_SERVER_ERROR, format!("{err}")).into_response();
        }
    }

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


    println!("{:?} {}", params.key, *ACCESS_KEY);


    if let Some(ref val) = params.key {
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

    let app_state = AppState {
        events,
        meta,
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
