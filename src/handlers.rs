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

// Response is same structure as request
//   to symbolise that a request has been
//   acknowledged
#[derive(Serialize)]
pub struct AddEntryResponse {
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
        new_state,
        start_timestamp,
    };

    (StatusCode::OK, Json(response)).into_response()
}

#[derive(Deserialize)]
pub struct FetchDataRequest {
    range: Option<u64>,
}

pub async fn fetch_data(
    Query(params): Query<FetchDataRequest>,
    State(state): State<AppState>,
) -> Response {
    // Very naive brute-force approach just to get the thing working
    let len = get_length(&state.meta);

    if len == 0 {
        return (StatusCode::BAD_REQUEST).into_response();
    }

    let mut cumulative = [0i64; STATE_COUNT];
    let mut old_state: Option<u8> = None;
    let mut old_timestamp: Option<i64> = None;

    let curr_time = Utc::now().timestamp_millis();
    let range_start = curr_time - params.range.unwrap_or(7) as i64 * 24 * 3600 * 1000;

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
