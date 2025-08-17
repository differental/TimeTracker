use axum::{
    Json,
    extract::State,
    http::StatusCode,
    response::{IntoResponse, Response},
};
use serde::Deserialize;
use sled::{IVec, Tree};

use crate::{
    constants::AppState,
    utils::{ivec_to_u64, to_ivec},
};

fn incr_length(meta: &mut Tree) -> u64 {
    // Inserts 0 if doesn't exist, returns new length
    let len = match meta.get(b"len").unwrap() {
        Some(val) => ivec_to_u64(val),
        None => 0,
    };

    let v = to_ivec((len + 1) as u64);

    // TO-DO: Handle Err(_) gracefully
    meta.insert(b"len", v).unwrap();

    len + 1
}

#[derive(Deserialize)]
pub struct AddEntryRequest {
    new_state: u8, // 0-indexed state
    start_timestamp: i64,
}

pub async fn add_entry(
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
