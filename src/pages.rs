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
    http::{StatusCode, header},
    response::{Html, IntoResponse, Response},
};
use chrono::{LocalResult, TimeZone, Utc};
use serde::Serialize;

use crate::{
    constants::{
        ACCESS_KEY, ALL_STATES_DETAILS, AppState, EMERGENCY_STATE_INDEX, IDLE_STATE, STATE_COUNT,
        StateDetail,
    },
    handlers::ASSET_VERSION,
    utils::{get_curr_state, get_length, log_corrupt_entry, read_from_value},
};

fn state_detail(curr_state: u8) -> StateDetail<'static> {
    if (curr_state as usize) < STATE_COUNT {
        ALL_STATES_DETAILS[curr_state as usize]
    } else {
        IDLE_STATE
    }
}

fn current_state_index(state: &AppState) -> Option<u8> {
    let curr_state = get_curr_state(state);
    ((curr_state as usize) < STATE_COUNT).then_some(curr_state)
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
pub struct Bootstrap<'a> {
    key: &'a str,
    page: &'a str,
    version: &'a str,
    states: [StateDetail<'a>; STATE_COUNT],
    idle_state: StateDetail<'a>,
    current_state: Option<u8>,
    elapsed_ms: i64,
    is_emergency: bool,
    emergency_state: usize,
}

fn bootstrap(page: &'static str, current_state: Option<u8>, elapsed_ms: i64) -> Bootstrap<'static> {
    Bootstrap {
        key: &ACCESS_KEY,
        page,
        version: env!("CARGO_PKG_VERSION"),
        states: ALL_STATES_DETAILS,
        idle_state: IDLE_STATE,
        current_state,
        elapsed_ms,
        is_emergency: current_state.is_some_and(|s| s as usize == EMERGENCY_STATE_INDEX),
        emergency_state: EMERGENCY_STATE_INDEX,
    }
}

fn page_response<T: Template>(page: &T) -> Response {
    let rendered = page.render().unwrap();

    (
        StatusCode::OK,
        [(header::CACHE_CONTROL, "no-store")],
        Html(rendered),
    )
        .into_response()
}

#[derive(Template)]
#[template(path = "index.html")]
struct IndexPageTemplate<'a> {
    key: &'a str,
    current_page: &'a str,
    states: [StateDetail<'a>; STATE_COUNT],
    current_state: StateDetail<'a>,
    is_emergency: bool,
    version: &'a str,
    asset_version: &'a str,
    bootstrap: Bootstrap<'a>,
}

fn render_idle_index() -> Response {
    let page = IndexPageTemplate {
        key: &ACCESS_KEY,
        current_page: "index",
        states: ALL_STATES_DETAILS,
        current_state: IDLE_STATE,
        is_emergency: false,
        version: env!("CARGO_PKG_VERSION"),
        asset_version: &ASSET_VERSION,
        bootstrap: bootstrap("index", None, 0),
    };

    page_response(&page)
}

pub async fn display_index(State(state): State<AppState>) -> impl IntoResponse {
    let last_id = get_length(&state.meta);

    if last_id == 0 {
        return render_idle_index();
    }

    let (curr_state, curr_starttime) = read_from_value(&state.events, last_id - 1);

    if curr_state as usize >= STATE_COUNT {
        log_corrupt_entry("display_index", last_id - 1, curr_state, curr_starttime);
        return render_idle_index();
    }

    let now = Utc::now();
    let starttime = match Utc.timestamp_millis_opt(curr_starttime) {
        LocalResult::Single(t) => t,
        _ => {
            log_corrupt_entry("display_index", last_id - 1, curr_state, curr_starttime);
            return render_idle_index();
        }
    };
    let duration = now - starttime;

    let page = IndexPageTemplate {
        key: &ACCESS_KEY,
        current_page: "index",
        states: ALL_STATES_DETAILS,
        current_state: ALL_STATES_DETAILS[curr_state as usize],
        is_emergency: curr_state as usize == EMERGENCY_STATE_INDEX,
        version: env!("CARGO_PKG_VERSION"),
        asset_version: &ASSET_VERSION,
        bootstrap: bootstrap("index", Some(curr_state), duration.num_milliseconds()),
    };

    page_response(&page)
}

#[derive(Template)]
#[template(path = "summary.html")]
struct SummaryPageTemplate<'a> {
    key: &'a str,
    current_page: &'a str,
    current_state: StateDetail<'a>,
    is_emergency: bool,
    version: &'a str,
    asset_version: &'a str,
    bootstrap: Bootstrap<'a>,
}

pub async fn display_summary(State(state): State<AppState>) -> Response {
    let curr_state = get_curr_state(&state);

    let page = SummaryPageTemplate {
        key: &ACCESS_KEY,
        current_page: "summary",
        current_state: state_detail(curr_state),
        is_emergency: curr_state as usize == EMERGENCY_STATE_INDEX,
        version: env!("CARGO_PKG_VERSION"),
        asset_version: &ASSET_VERSION,
        bootstrap: bootstrap("summary", current_state_index(&state), 0),
    };

    page_response(&page)
}

#[derive(Template)]
#[template(path = "explanations.html")]
struct ExplanationPageTemplate<'a> {
    key: &'a str,
    current_page: &'a str,
    states: [StateDetail<'a>; STATE_COUNT],
    current_state: StateDetail<'a>,
    is_emergency: bool,
    version: &'a str,
    asset_version: &'a str,
    bootstrap: Bootstrap<'a>,
}

pub async fn display_explanations(State(state): State<AppState>) -> Response {
    let curr_state = get_curr_state(&state);

    let page = ExplanationPageTemplate {
        key: &ACCESS_KEY,
        current_page: "explanations",
        states: ALL_STATES_DETAILS,
        current_state: state_detail(curr_state),
        is_emergency: curr_state as usize == EMERGENCY_STATE_INDEX,
        version: env!("CARGO_PKG_VERSION"),
        asset_version: &ASSET_VERSION,
        bootstrap: bootstrap("explanations", current_state_index(&state), 0),
    };

    page_response(&page)
}

#[derive(Template)]
#[template(path = "recents.html")]
struct RecentsPageTemplate<'a> {
    key: &'a str,
    current_page: &'a str,
    current_state: StateDetail<'a>,
    is_emergency: bool,
    version: &'a str,
    asset_version: &'a str,
    bootstrap: Bootstrap<'a>,
}

pub async fn display_recents(State(state): State<AppState>) -> Response {
    let curr_state = get_curr_state(&state);

    let page = RecentsPageTemplate {
        key: &ACCESS_KEY,
        current_page: "recents",
        current_state: state_detail(curr_state),
        is_emergency: curr_state as usize == EMERGENCY_STATE_INDEX,
        version: env!("CARGO_PKG_VERSION"),
        asset_version: &ASSET_VERSION,
        bootstrap: bootstrap("recents", current_state_index(&state), 0),
    };

    page_response(&page)
}
