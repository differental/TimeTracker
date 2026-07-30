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
    extract::{Path, Query, RawQuery, State},
    http::{HeaderMap, HeaderValue, StatusCode, header},
    response::{IntoResponse, Response},
};
use chrono::{FixedOffset, Utc};
use mime_guess::from_path;
use rust_embed::RustEmbed;
use serde::{Deserialize, Serialize};
use sled::{IVec, Transactional, transaction::TransactionResult};
use std::sync::LazyLock;

use crate::{
    constants::{ALL_STATES_DETAILS, AppState, STATE_COUNT},
    predictor::{ActivityPredictor, Configuration, TrainingEntry},
    utils::{
        get_length, incr_length, is_reasonable_timestamp, is_valid_timestamp, ivec_to_u64,
        log_corrupt_entry, read_from_value, to_ivec, try_read_from_value,
    },
};

#[derive(RustEmbed)]
#[folder = "static/"]
struct Assets;

fn hex16(bytes: &[u8]) -> String {
    bytes[..8].iter().map(|b| format!("{b:02x}")).collect()
}

pub static ASSET_VERSION: LazyLock<String> = LazyLock::new(|| {
    let mut names = Assets::iter().collect::<Vec<_>>();
    names.sort();

    let mut hash: u64 = 0xcbf2_9ce4_8422_2325;
    for name in &names {
        let Some(file) = Assets::get(name) else {
            continue;
        };
        let digest = file.metadata.sha256_hash();
        for byte in name.as_bytes().iter().chain(digest.iter()) {
            hash = (hash ^ *byte as u64).wrapping_mul(0x100_0000_01b3);
        }
    }

    format!("{hash:016x}")
});

fn assets_are_embedded_at_compile_time() -> bool {
    !cfg!(debug_assertions)
}

fn is_fingerprinted(query: Option<&str>) -> bool {
    assets_are_embedded_at_compile_time()
        && query.is_some_and(|q| {
            q.split('&')
                .any(|pair| pair.strip_prefix("v=") == Some(ASSET_VERSION.as_str()))
        })
}

pub async fn serve_embedded_assets(
    Path(file): Path<String>,
    RawQuery(query): RawQuery,
    headers: HeaderMap,
) -> Response {
    let Some(content) = Assets::get(&file) else {
        return StatusCode::NOT_FOUND.into_response();
    };

    let cache_control = if is_fingerprinted(query.as_deref()) {
        "public, max-age=31536000, immutable"
    } else {
        "public, max-age=0, must-revalidate"
    };
    let etag = format!("\"{}\"", hex16(&content.metadata.sha256_hash()));

    let mut out = HeaderMap::new();
    out.insert(
        header::CACHE_CONTROL,
        HeaderValue::from_static(cache_control),
    );
    out.insert(header::ETAG, HeaderValue::from_str(&etag).unwrap());

    let fresh = headers
        .get(header::IF_NONE_MATCH)
        .and_then(|v| v.to_str().ok())
        .is_some_and(|v| v.split(',').any(|candidate| candidate.trim() == etag));

    if fresh {
        return (StatusCode::NOT_MODIFIED, out).into_response();
    }

    let mime = from_path(&file).first_or_octet_stream();
    out.insert(
        header::CONTENT_TYPE,
        HeaderValue::from_str(mime.as_ref()).unwrap(),
    );

    (StatusCode::OK, out, content.data.into_owned()).into_response()
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

    // Always enforced, even for forced writes. An out-of-range state index would
    // later panic when rendering, and an unreasonable timestamp corrupts the DB
    // (this is what force must never be allowed to slip through).
    if new_state as usize >= STATE_COUNT {
        return (StatusCode::BAD_REQUEST, "Bad request: Invalid state index").into_response();
    }
    if !is_reasonable_timestamp(start_timestamp, now) {
        return (
            StatusCode::BAD_REQUEST,
            "Bad request: Unreasonable start timestamp",
        )
            .into_response();
    }

    // Soft check, bypassable with force: a normal add records the present moment
    // (within a few seconds). Forced writes may deliberately backdate the start.
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
        // Never bypassed, even with force: entries must stay ordered by start
        // timestamp, so a new entry cannot begin before the current one.
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
    force: Option<bool>,
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
        force,
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

    let now = Utc::now().timestamp_millis();

    // Always enforced, even for forced writes: a supplied state index must be
    // valid and a supplied timestamp must be reasonable.
    if new_state.is_some_and(|ns| ns as usize >= STATE_COUNT) {
        return (StatusCode::BAD_REQUEST, "Bad request: Invalid state index").into_response();
    }
    if start_timestamp.is_some_and(|ts| !is_reasonable_timestamp(ts, now)) {
        return (
            StatusCode::BAD_REQUEST,
            "Bad request: Unreasonable start timestamp",
        )
            .into_response();
    }

    // Never bypassed, even with force: the edited start must stay ordered
    // between its neighbouring entries.
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

    // Soft check, bypassable with force: only relevant when the state is being
    // changed. Reject an edit that would leave two consecutive entries sharing a
    // state (a redundant, zero-information segment) unless the caller forces it.
    let bypass = force == Some(true);
    if let Some(ns) = new_state {
        if !bypass && entry_idx > 0 && read_from_value(&state.events, entry_idx - 1).0 == ns {
            return (
                StatusCode::BAD_REQUEST,
                "Bad request: New state same as previous entry",
            )
                .into_response();
        }
        if !bypass
            && entry_idx < length - 1
            && read_from_value(&state.events, entry_idx + 1).0 == ns
        {
            return (
                StatusCode::BAD_REQUEST,
                "Bad request: New state same as next entry",
            )
                .into_response();
        }
    }

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

    if !is_valid_timestamp(start_timestamp) {
        log_corrupt_entry("get_entry", entry_idx, new_state, start_timestamp);
    }

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
        if !is_valid_timestamp(timestamp) {
            log_corrupt_entry("fetch_summary_data", i, state, timestamp);
        }
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
pub struct SuggestRequest {
    /// How many activities to suggest. Defaults to 3.
    limit: Option<usize>,
    /// The client's UTC offset in minutes, east of Greenwich. Weekday and
    /// hour-of-day context is derived in this timezone, so a client in a
    /// non-UTC zone must pass it or the time-of-day patterns will be skewed.
    tz_offset: Option<i32>,
    /// Moment to predict for, in epoch milliseconds. Defaults to now.
    at: Option<i64>,
    /// State to predict away from. Defaults to the latest recorded state, and
    /// is never itself suggested.
    current_state: Option<u8>,
    /// How many of the most recent entries to train on. Defaults to 10000.
    max_entries: Option<usize>,
}

#[derive(Serialize)]
pub struct Suggestion {
    state: u8,
    name: &'static str,
    emoji: &'static str,
    colour: &'static str,
}

#[derive(Serialize)]
pub struct SuggestResponse {
    at: i64,
    current_state: Option<u8>,
    trained_on: usize,
    suggestions: Vec<Suggestion>,
}

const MAX_TZ_OFFSET_MINUTES: i32 = 14 * 60;
const MAX_TRAINING_ENTRIES: usize = 100_000;

/// Suggests the activities most likely to come next, using the TAGE predictor in
/// [`crate::predictor`]. Training happens per request over the tail of the log,
/// which keeps the clients stateless — they no longer need to download the whole
/// history to predict locally.
pub async fn suggest_next_states(
    Query(params): Query<SuggestRequest>,
    State(state): State<AppState>,
) -> Response {
    let SuggestRequest {
        limit,
        tz_offset,
        at,
        current_state,
        max_entries,
    } = params;

    let limit = limit.unwrap_or(3);
    if limit == 0 || limit > STATE_COUNT {
        return (StatusCode::BAD_REQUEST, "Bad request: Invalid limit").into_response();
    }

    let tz_offset = tz_offset.unwrap_or(0);
    let Some(offset) = (tz_offset.abs() <= MAX_TZ_OFFSET_MINUTES)
        .then(|| FixedOffset::east_opt(tz_offset * 60))
        .flatten()
    else {
        return (StatusCode::BAD_REQUEST, "Bad request: Invalid tz_offset").into_response();
    };

    let at = at.unwrap_or_else(|| Utc::now().timestamp_millis());
    if !is_valid_timestamp(at) {
        return (StatusCode::BAD_REQUEST, "Bad request: Invalid timestamp").into_response();
    }

    if current_state.is_some_and(|s| s as usize >= STATE_COUNT) {
        return (StatusCode::BAD_REQUEST, "Bad request: Invalid state index").into_response();
    }

    let max_entries = max_entries.unwrap_or(10_000);
    if max_entries == 0 || max_entries > MAX_TRAINING_ENTRIES {
        return (StatusCode::BAD_REQUEST, "Bad request: Invalid max_entries").into_response();
    }

    // Reading the log and training over it is CPU-bound and can span tens of
    // thousands of entries, so keep it off the async runtime's worker threads.
    let predicted = tokio::task::spawn_blocking(move || {
        let length = get_length(&state.meta);
        let start = length.saturating_sub(max_entries as u64);

        let mut entries = Vec::with_capacity((length - start) as usize);
        for i in start..length {
            let (state_id, timestamp) = read_from_value(&state.events, i);
            // A state index out of range or an unrepresentable timestamp can't be
            // placed in the training sequence at all, so log and drop it rather
            // than let one corrupt row skew every prediction.
            if state_id as usize >= STATE_COUNT || !is_valid_timestamp(timestamp) {
                log_corrupt_entry("suggest_next_states", i, state_id, timestamp);
                continue;
            }
            entries.push(TrainingEntry {
                state_id: state_id as usize,
                start_timestamp: timestamp,
            });
        }

        let current_state = current_state.or_else(|| entries.last().map(|e| e.state_id as u8));

        let configuration = Configuration {
            maximum_training_entries: max_entries,
            ..Configuration::default()
        };
        let trained_on = entries.len();
        let predictor = ActivityPredictor::new(&entries, offset, configuration);
        let states = predictor.predictions(at, current_state.map(|s| s as usize), limit);

        (current_state, trained_on, states)
    })
    .await;

    let (current_state, trained_on, states) = match predicted {
        Ok(result) => result,
        Err(err) => {
            println!("{err:?}");
            return (StatusCode::INTERNAL_SERVER_ERROR, format!("{err}")).into_response();
        }
    };

    let suggestions = states
        .into_iter()
        .map(|state_id| {
            let detail = ALL_STATES_DETAILS[state_id];
            Suggestion {
                state: state_id as u8,
                name: detail.name,
                emoji: detail.emoji,
                colour: detail.colour,
            }
        })
        .collect();

    (
        StatusCode::OK,
        Json(SuggestResponse {
            at,
            current_state,
            trained_on,
            suggestions,
        }),
    )
        .into_response()
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

    let mut output = Vec::<(u8, i64)>::new();
    for i in ((length - count)..=(length - 1)).rev() {
        let (s, t) = read_from_value(&state.events, i);
        if !is_valid_timestamp(t) {
            log_corrupt_entry("fetch_recent_states", i, s, t);
        }
        output.push((s, t));
        if t < range_start {
            break;
        }
    }

    (StatusCode::OK, Json(output)).into_response()
}

pub const EXPORT_FORMAT_VERSION: u32 = 1;

#[derive(Serialize, Deserialize)]
pub struct ExportEntry {
    entry_idx: u64,
    new_state: u8,
    start_timestamp: i64,
}

#[derive(Serialize)]
pub struct ExportResponse {
    version: u32,
    exported_at: i64,
    count: u64,
    entries: Vec<ExportEntry>,
}

pub async fn export_data(State(state): State<AppState>) -> Response {
    let length = get_length(&state.meta);

    let mut entries: Vec<ExportEntry> = Vec::new();

    for i in 0..length {
        let Some((new_state, start_timestamp)) = try_read_from_value(&state.events, i) else {
            eprintln!("export_data: skipping unreadable entry at index {i}");
            continue;
        };

        if !is_valid_timestamp(start_timestamp) {
            log_corrupt_entry("export_data", i, new_state, start_timestamp);
        }

        entries.push(ExportEntry {
            entry_idx: entries.len() as u64,
            new_state,
            start_timestamp,
        });
    }

    let response = ExportResponse {
        version: EXPORT_FORMAT_VERSION,
        exported_at: Utc::now().timestamp_millis(),
        count: entries.len() as u64,
        entries,
    };

    (StatusCode::OK, Json(response)).into_response()
}

#[derive(Deserialize)]
pub struct ImportRequest {
    version: u32,
    count: Option<u64>,
    entries: Vec<ExportEntry>,
    force: Option<bool>,
}

#[derive(Serialize)]
pub struct ImportResponse {
    imported: u64,
    previous_length: u64,
}

fn validate_import(entries: &[ExportEntry], force: bool, now: i64) -> Result<(), String> {
    if entries.is_empty() && !force {
        return Err("Bad request: Refusing to import an empty database without force".to_string());
    }

    for (i, entry) in entries.iter().enumerate() {
        if entry.entry_idx != i as u64 {
            return Err(format!("Bad request: Entry index mismatch at entry {i}"));
        }

        if entry.new_state as usize >= STATE_COUNT {
            return Err(format!("Bad request: Invalid state index at entry {i}"));
        }

        if !is_reasonable_timestamp(entry.start_timestamp, now) {
            return Err(format!(
                "Bad request: Unreasonable start timestamp at entry {i}"
            ));
        }

        if i > 0 {
            let previous = &entries[i - 1];

            if entry.start_timestamp < previous.start_timestamp {
                return Err(format!(
                    "Bad request: Entries not ordered by start timestamp at entry {i}"
                ));
            }

            if !force && entry.new_state == previous.new_state {
                return Err(format!(
                    "Bad request: Consecutive entries share a state at entry {i}"
                ));
            }
        }
    }

    Ok(())
}

pub async fn import_data(
    State(state): State<AppState>,
    Json(payload): Json<ImportRequest>,
) -> Response {
    let ImportRequest {
        version,
        count,
        entries,
        force,
    } = payload;

    if version != EXPORT_FORMAT_VERSION {
        return (
            StatusCode::BAD_REQUEST,
            "Bad request: Unsupported export format version",
        )
            .into_response();
    }

    if count.is_some_and(|c| c != entries.len() as u64) {
        return (
            StatusCode::BAD_REQUEST,
            "Bad request: Count does not match entries length",
        )
            .into_response();
    }

    let now = Utc::now().timestamp_millis();

    if let Err(err) = validate_import(&entries, force == Some(true), now) {
        return (StatusCode::BAD_REQUEST, err).into_response();
    }

    let previous_length = get_length(&state.meta);
    let new_length = entries.len() as u64;

    let existing = state
        .events
        .iter()
        .keys()
        .filter_map(|key| key.ok())
        .map(ivec_to_u64)
        .collect::<Vec<u64>>();

    let result: TransactionResult<(), sled::Error> =
        (&state.events, &state.meta).transaction(|(tx_events, tx_meta)| {
            for key in &existing {
                if *key >= new_length {
                    tx_events.remove(to_ivec(*key))?;
                }
            }

            for (i, entry) in entries.iter().enumerate() {
                let mut bytes = [0u8; 9];
                bytes[0] = entry.new_state;
                bytes[1..].copy_from_slice(&entry.start_timestamp.to_ne_bytes());
                tx_events.insert(to_ivec(i as u64), IVec::from(&bytes))?;
            }

            tx_meta.insert(b"len", to_ivec(new_length))?;

            Ok(())
        });

    if let Err(err) = result {
        println!("{err:?}");
        return (StatusCode::INTERNAL_SERVER_ERROR, format!("{err}")).into_response();
    }

    for tree in [&state.events, &state.meta] {
        if let Err(err) = tree.flush() {
            println!("{err:?}");
            return (StatusCode::INTERNAL_SERVER_ERROR, format!("{err}")).into_response();
        }
    }

    (
        StatusCode::OK,
        Json(ImportResponse {
            imported: new_length,
            previous_length,
        }),
    )
        .into_response()
}
