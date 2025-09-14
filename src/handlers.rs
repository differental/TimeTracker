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

use axum::{
    Json,
    extract::{Path, Query, State},
    http::{HeaderMap, HeaderValue, StatusCode},
    response::{IntoResponse, Response},
};
use chrono::Utc;
use mime_guess::from_path;
use rust_embed::RustEmbed;
use serde::{Deserialize, Serialize};
use sled::IVec;

use crate::{
    constants::{AppState, STATE_COUNT},
    utils::{get_length, incr_length, read_from_value, to_ivec},
};

#[derive(RustEmbed)]
#[folder = "static/"]
struct Assets;

pub async fn serve_embedded_assets(Path(file): Path<String>) -> Response {
    match Assets::get(&file) {
        Some(content) => {
            let body = content.data.into_owned();
            let mime = from_path(&file).first_or_octet_stream();

            let mut headers = HeaderMap::new();
            headers.insert(
                "Content-Type",
                HeaderValue::from_str(mime.as_ref()).unwrap(),
            );

            (StatusCode::OK, headers, body).into_response()
        }
        None => StatusCode::NOT_FOUND.into_response(),
    }
}

#[derive(Deserialize)]
pub struct AddEntryRequest {
    new_state: u8, // 0-indexed state
    start_timestamp: i64,
    force: Option<bool>,
}

#[derive(Serialize)]
pub struct AddEntryResponse {
    entry_idx: u64,
    new_state: u8,
    start_timestamp: i64,
}

pub async fn add_entry(
    State(state): State<AppState>,
    Json(payload): Json<AddEntryRequest>,
) -> Response {
    let AddEntryRequest {
        new_state,
        start_timestamp,
        force,
    } = payload;

    let now = Utc::now().timestamp_millis();
    if force != Some(true) && (start_timestamp < now - 5000 || start_timestamp > now) {
        return (StatusCode::BAD_REQUEST, "Bad request: Wrong timestamp").into_response();
    }

    let new_key = get_length(&state.meta);

    if new_key >= 1 {
        let (curr_state, curr_starttime) = read_from_value(&state.events, new_key - 1);

        if curr_state == new_state {
            return (
                StatusCode::BAD_REQUEST,
                "Bad request: New state same as current state",
            )
                .into_response();
        }
        if start_timestamp < curr_starttime {
            return (
                StatusCode::BAD_REQUEST,
                "Bad request: New starttime earlier than current starttime",
            )
                .into_response();
        }
    }

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

    incr_length(&state.meta);

    let response = AddEntryResponse {
        entry_idx: new_key,
        new_state,
        start_timestamp,
    };

    (StatusCode::OK, Json(response)).into_response()
}

#[derive(Deserialize)]
pub struct UpdateEntryRequest {
    new_state: Option<u8>,
    start_timestamp: Option<i64>,
}

#[derive(Serialize)]
pub struct UpdateEntryResponse {
    entry_idx: u64,
    new_state: u8,
    start_timestamp: i64,
}

pub async fn update_entry(
    Path(entry_idx): Path<u64>,
    State(state): State<AppState>,
    Json(payload): Json<UpdateEntryRequest>,
) -> Response {
    let UpdateEntryRequest {
        new_state,
        start_timestamp,
    } = payload;

    // perform basic validation
    let length = get_length(&state.meta);

    if entry_idx >= length {
        return (
            StatusCode::BAD_REQUEST,
            "Bad request: Entry index out of range",
        )
            .into_response();
    }

    if start_timestamp.is_none() && new_state.is_none() {
        return (StatusCode::BAD_REQUEST, "Bad request: No changes specified").into_response();
    }

    if let Some(curr_start_time) = start_timestamp {
        if entry_idx > 0 {
            let (_, last_start_time) = read_from_value(&state.events, entry_idx - 1);
            if curr_start_time < last_start_time {
                return (
                    StatusCode::BAD_REQUEST,
                    "Bad request: New starttime earlier than previous event",
                )
                    .into_response();
            }
        }

        if entry_idx < length - 1 {
            let (_, next_start_time) = read_from_value(&state.events, entry_idx + 1);
            if curr_start_time > next_start_time {
                return (
                    StatusCode::BAD_REQUEST,
                    "Bad request: New starttime later than next event",
                )
                    .into_response();
            }
        }
    }

    let (original_new_state, original_start_timestamp) = read_from_value(&state.events, entry_idx);
    let new_state = new_state.unwrap_or(original_new_state);
    let start_timestamp = start_timestamp.unwrap_or(original_start_timestamp);

    // Inserted element: First byte is new_state, next 8 bytes are start_timestamp
    let mut bytes = [0u8; 9];
    bytes[0] = new_state;
    bytes[1..].copy_from_slice(&start_timestamp.to_ne_bytes());

    match state.events.insert(to_ivec(entry_idx), IVec::from(&bytes)) {
        Ok(_) => (),
        Err(err) => {
            println!("{err:?}");
            return (StatusCode::INTERNAL_SERVER_ERROR, format!("{err}")).into_response();
        }
    }

    let response = UpdateEntryResponse {
        entry_idx,
        new_state,
        start_timestamp,
    };

    (StatusCode::OK, Json(response)).into_response()
}

#[derive(Serialize)]
pub struct GetEntryResponse {
    entry_idx: u64,
    new_state: u8,
    start_timestamp: i64,
}

pub async fn get_entry(Path(entry_idx): Path<u64>, State(state): State<AppState>) -> Response {
    let length = get_length(&state.meta);

    if entry_idx >= length {
        return (
            StatusCode::BAD_REQUEST,
            "Bad request: Entry index out of range",
        )
            .into_response();
    }

    let (new_state, start_timestamp) = read_from_value(&state.events, entry_idx);

    (
        StatusCode::OK,
        Json(GetEntryResponse {
            entry_idx,
            new_state,
            start_timestamp,
        }),
    )
        .into_response()
}

#[derive(Deserialize)]
pub struct FetchSummaryDataRequest {
    days: Option<u32>,
}

pub async fn fetch_summary_data(
    Query(params): Query<FetchSummaryDataRequest>,
    State(state): State<AppState>,
) -> Response {
    // Very naive brute-force approach just to get the thing working
    let len = get_length(&state.meta);

    let mut cumulative = [0i64; STATE_COUNT];

    if len == 0 {
        return (StatusCode::OK, Json(cumulative)).into_response();
    }

    let mut old_state: Option<u8> = None;
    let mut old_timestamp: Option<i64> = None;

    let curr_time = Utc::now().timestamp_millis();
    let range_start = curr_time - params.days.unwrap_or(7u32) as i64 * 24 * 3600 * 1000;

    let mut pre_range_start_state: Option<u8> = None;

    for i in 0..len {
        let (state, timestamp) = read_from_value(&state.events, i);
        if timestamp < range_start {
            pre_range_start_state = Some(state);
            continue;
        }

        if let Some(val) = old_state {
            cumulative[val as usize] += timestamp - old_timestamp.unwrap();
        } else if let Some(pre_start_state) = pre_range_start_state {
            cumulative[pre_start_state as usize] += timestamp - range_start;
        }
        old_timestamp = Some(timestamp);
        old_state = Some(state);
    }

    if let Some(val) = old_state {
        cumulative[val as usize] += curr_time - old_timestamp.unwrap();
    } else if let Some(pre_start_state) = pre_range_start_state {
        cumulative[pre_start_state as usize] += curr_time - range_start;
    }

    (StatusCode::OK, Json(cumulative)).into_response()
}

pub async fn fetch_length(State(state): State<AppState>) -> Response {
    let length = get_length(&state.meta);

    (StatusCode::OK, Json(length)).into_response()
}

#[derive(Deserialize)]
pub struct ForceSetLengthRequest {
    new_length: u64,
}

pub async fn force_set_length(
    State(state): State<AppState>,
    Json(payload): Json<ForceSetLengthRequest>,
) -> Response {
    let ForceSetLengthRequest { new_length } = payload;

    let v = to_ivec(new_length);

    // TO-DO: Handle Err(_) gracefully
    state.meta.insert(b"len", v).unwrap();

    (StatusCode::OK, Json(new_length)).into_response()
}

#[derive(Deserialize)]
pub struct FetchRecentsRequest {
    count: Option<u64>,
    days: Option<u32>,
}

pub async fn fetch_recent_states(
    Query(params): Query<FetchRecentsRequest>,
    State(state): State<AppState>,
) -> Response {
    let length = get_length(&state.meta);

    // If the user doesn't pass in either param, we use these very large defaults.
    let count = length.min(params.count.unwrap_or(300u64));
    let days = params.days.unwrap_or(30u32) as i64;

    // User is guarded against specifying params.count = 0 by frontend, but we
    //   should return an empty vector rather than panic with out-of-bounds access.
    // This also happens if length == 0.
    if count == 0 {
        return (StatusCode::OK, Json(Vec::<(u8, i64)>::new())).into_response();
    }

    let curr_time = Utc::now().timestamp_millis();
    let range_start = curr_time - days * 24 * 3600 * 1000;

    let output = ((length - count)..=(length - 1))
        .rev()
        .map(|i| read_from_value(&state.events, i))
        .take_while(|(_, t)| *t >= range_start)
        .collect::<Vec<(u8, i64)>>();

    (StatusCode::OK, Json(output)).into_response()
}
