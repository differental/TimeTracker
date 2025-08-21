use askama::Template;
use axum::{
    extract::State,
    http::StatusCode,
    response::{Html, IntoResponse, Response},
};
use chrono::{TimeZone, Utc};

use crate::{
    constants::{ACCESS_KEY, ALL_STATES_DETAILS, AppState, IDLE_STATE, STATE_COUNT, StateDetail},
    utils::{get_length, read_from_value},
};

#[derive(Template)]
#[template(path = "index.html")]
struct IndexPageTemplate<'a> {
    key: &'a str,
    current_page: &'a str,
    states: [StateDetail<'a>; STATE_COUNT],
    current_state: StateDetail<'a>,
    elapsed_ms: i64,
    version: &'a str,
}

pub async fn display_index(State(state): State<AppState>) -> impl IntoResponse {
    let last_id = get_length(&state.meta);

    if last_id == 0 {
        let page = IndexPageTemplate {
            key: &*ACCESS_KEY,
            current_page: "index",
            states: ALL_STATES_DETAILS,
            current_state: IDLE_STATE,
            elapsed_ms: 0,
            version: env!("CARGO_PKG_VERSION"),
        };

        let rendered = page.render().unwrap();
        return (StatusCode::OK, Html(rendered)).into_response();
    }

    let (curr_state, curr_starttime) = read_from_value(&state.events, last_id - 1);

    let now = Utc::now();
    let starttime = Utc.timestamp_millis_opt(curr_starttime).unwrap();
    let duration = now - starttime;

    let page = IndexPageTemplate {
        key: &*ACCESS_KEY,
        current_page: "index",
        states: ALL_STATES_DETAILS,
        current_state: ALL_STATES_DETAILS[curr_state as usize],
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
    current_page: &'a str,
    states: [StateDetail<'a>; STATE_COUNT],
    version: &'a str,
}

pub async fn display_summary() -> Response {
    let page = SummaryPageTemplate {
        key: &*ACCESS_KEY,
        current_page: "summary",
        states: ALL_STATES_DETAILS,
        version: env!("CARGO_PKG_VERSION"),
    };

    let rendered = page.render().unwrap();
    (StatusCode::OK, Html(rendered)).into_response()
}

#[derive(Template)]
#[template(path = "explanations.html")]
struct ExplanationPageTemplate<'a> {
    key: &'a str,
    current_page: &'a str,
    states: [StateDetail<'a>; STATE_COUNT],
    version: &'a str,
}

pub async fn display_explanations() -> Response {
    let page = ExplanationPageTemplate {
        key: &*ACCESS_KEY,
        current_page: "explanations",
        states: ALL_STATES_DETAILS,
        version: env!("CARGO_PKG_VERSION"),
    };

    let rendered = page.render().unwrap();
    (StatusCode::OK, Html(rendered)).into_response()
}
