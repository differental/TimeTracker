use askama::Template;
use axum::{
    extract::{Query, State},
    http::StatusCode,
    response::{Html, IntoResponse, Response},
};
use chrono::{TimeZone, Utc};
use serde::Deserialize;
use sled::Tree;

use crate::{
    constants::{ACCESS_KEY, AppState, STATES},
    utils::{ivec_to_u64, to_ivec},
};

fn get_length(meta: &Tree) -> u64 {
    match meta.get(b"len").unwrap() {
        Some(val) => ivec_to_u64(val),
        None => {
            // TO-DO: Handle Err(_) gracefully
            meta.insert(b"len", to_ivec(0u64)).unwrap();
            0
        }
    }
}

#[derive(Template)]
#[template(path = "index.html")]
struct IndexPageTemplate<'a> {
    key: &'a str,
    states: [&'a str; 10],
    current_page: &'a str,
    current_state: &'a str,
    elapsed_hms: String,
    elapsed_ms: i64,
    version: &'a str
}

pub async fn display_index(State(state): State<AppState>) -> impl IntoResponse {
    let last_id = get_length(&state.meta);

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
        current_page: "index",
        current_state: STATES[curr_state as usize],
        elapsed_hms,
        elapsed_ms: duration.num_milliseconds(),
        version: env!("CARGO_PKG_VERSION")
    };

    let rendered = page.render().unwrap();
    (StatusCode::OK, Html(rendered)).into_response()
}

#[derive(Template)]
#[template(path = "summary.html")]
struct SummaryPageTemplate<'a> {
    key: &'a str,
    states: [&'a str; 10],
    current_page: &'a str,
    range_label: String,
    version: &'a str
}

#[derive(Deserialize)]
pub struct SummaryPageParams {
    // No. of days, defaults to 7
    range: Option<u64>,
}

pub async fn display_summary(Query(params): Query<SummaryPageParams>) -> Response {
    let page = SummaryPageTemplate {
        key: &*ACCESS_KEY,
        states: STATES,
        current_page: "summary",
        range_label: params.range.unwrap_or(7).to_string(),
        version: env!("CARGO_PKG_VERSION")
    };

    let rendered = page.render().unwrap();
    (StatusCode::OK, Html(rendered)).into_response()
}
