use askama::Template;
use axum::{
    extract::State,
    http::StatusCode,
    response::{Html, IntoResponse, Response},
};
use chrono::{TimeZone, Utc};

use crate::{
    constants::{ACCESS_KEY, AppState, STATES},
    utils::{get_length, read_from_value},
};

#[derive(Template)]
#[template(path = "index.html")]
struct IndexPageTemplate<'a> {
    key: &'a str,
    states: [&'a str; 10],
    current_page: &'a str,
    current_state: &'a str,
    elapsed_hms: String,
    elapsed_ms: i64,
    version: &'a str,
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

    let (curr_state, curr_starttime) = read_from_value(&state.events, last_id - 1);

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
        version: env!("CARGO_PKG_VERSION"),
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
    version: &'a str,
}

pub async fn display_summary() -> Response {
    let page = SummaryPageTemplate {
        key: &*ACCESS_KEY,
        states: STATES,
        current_page: "summary",
        version: env!("CARGO_PKG_VERSION"),
    };

    let rendered = page.render().unwrap();
    (StatusCode::OK, Html(rendered)).into_response()
}
