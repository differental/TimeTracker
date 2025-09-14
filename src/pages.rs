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

use askama::Template;
use axum::{
    extract::State,
    http::StatusCode,
    response::{Html, IntoResponse, Response},
};
use chrono::{TimeZone, Utc};

use crate::{
    constants::{
        ACCESS_KEY, ALL_STATES_DETAILS, AppState, EMERGENCY_STATE_INDEX, IDLE_STATE, STATE_COUNT,
        StateDetail,
    },
    utils::{get_curr_state, get_length, read_from_value},
};

#[derive(Template)]
#[template(path = "index.html")]
struct IndexPageTemplate<'a> {
    key: &'a str,
    current_page: &'a str,
    states: [StateDetail<'a>; STATE_COUNT],
    current_state: StateDetail<'a>,
    elapsed_ms: i64,
    is_emergency: bool,
    version: &'a str,
}

pub async fn display_index(State(state): State<AppState>) -> impl IntoResponse {
    let last_id = get_length(&state.meta);

    if last_id == 0 {
        let page = IndexPageTemplate {
            key: &ACCESS_KEY,
            current_page: "index",
            states: ALL_STATES_DETAILS,
            current_state: IDLE_STATE,
            elapsed_ms: 0,
            is_emergency: false,
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
        key: &ACCESS_KEY,
        current_page: "index",
        states: ALL_STATES_DETAILS,
        current_state: ALL_STATES_DETAILS[curr_state as usize],
        elapsed_ms: duration.num_milliseconds(),
        is_emergency: curr_state as usize == EMERGENCY_STATE_INDEX,
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
    is_emergency: bool,
    version: &'a str,
}

pub async fn display_summary(State(state): State<AppState>) -> Response {
    let page = SummaryPageTemplate {
        key: &ACCESS_KEY,
        current_page: "summary",
        states: ALL_STATES_DETAILS,
        is_emergency: get_curr_state(&state) as usize == EMERGENCY_STATE_INDEX,
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
    is_emergency: bool,
    version: &'a str,
}

pub async fn display_explanations(State(state): State<AppState>) -> Response {
    let page = ExplanationPageTemplate {
        key: &ACCESS_KEY,
        current_page: "explanations",
        states: ALL_STATES_DETAILS,
        is_emergency: get_curr_state(&state) as usize == EMERGENCY_STATE_INDEX,
        version: env!("CARGO_PKG_VERSION"),
    };

    let rendered = page.render().unwrap();
    (StatusCode::OK, Html(rendered)).into_response()
}

#[derive(Template)]
#[template(path = "recents.html")]
struct RecentsPageTemplate<'a> {
    key: &'a str,
    current_page: &'a str,
    states: [StateDetail<'a>; STATE_COUNT],
    is_emergency: bool,
    version: &'a str,
}

pub async fn display_recents(State(state): State<AppState>) -> Response {
    let page = RecentsPageTemplate {
        key: &ACCESS_KEY,
        current_page: "recents",
        states: ALL_STATES_DETAILS,
        is_emergency: get_curr_state(&state) as usize == EMERGENCY_STATE_INDEX,
        version: env!("CARGO_PKG_VERSION"),
    };

    let rendered = page.render().unwrap();
    (StatusCode::OK, Html(rendered)).into_response()
}
