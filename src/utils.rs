use num::traits::ToBytes;
use sled::{IVec, Tree};

use crate::constants::AppState;

pub fn ivec_to_u64(v: IVec) -> u64 {
    let slice = v.as_ref();
    let mut bytes = [0u8; 8];
    bytes.copy_from_slice(&slice[0..8]);
    u64::from_ne_bytes(bytes)
}

pub fn to_ivec<T: ToBytes>(n: T) -> IVec
where
    IVec: for<'a> From<&'a T::Bytes>,
{
    // There's gotta be some way to not express this in such an ugly way...
    let bytes = n.to_ne_bytes();
    IVec::from(&bytes)
}

pub fn get_length(meta: &Tree) -> u64 {
    match meta.get(b"len").unwrap() {
        Some(val) => ivec_to_u64(val),
        None => {
            // TO-DO: Handle Err(_) gracefully
            meta.insert(b"len", to_ivec(0u64)).unwrap();
            0
        }
    }
}

pub fn incr_length(meta: &Tree) -> u64 {
    // Inserts 0 if doesn't exist, returns new length
    let len = match meta.get(b"len").unwrap() {
        Some(val) => ivec_to_u64(val),
        None => 0,
    };

    let v = to_ivec(len + 1);

    // TO-DO: Handle Err(_) gracefully
    meta.insert(b"len", v).unwrap();

    len + 1
}

pub fn read_from_value(events: &Tree, id: u64) -> (u8, i64) {
    // TO-DO: Handle None and Err(_) gracefully
    let bytes = events.get(id.to_ne_bytes()).unwrap().unwrap();
    let state = u8::from_ne_bytes([bytes[0]]);
    let mut time_bytes = [0u8; 8];
    time_bytes.copy_from_slice(&bytes[1..]);
    let starttime = i64::from_ne_bytes(time_bytes);
    (state, starttime)
}

pub fn get_curr_state(state: &AppState) -> u8 {
    // Returns current state, or if there's no state, u8::MAX
    let length = get_length(&state.meta);
    if length >= 1 {
        read_from_value(&state.events, length - 1).0
    } else {
        u8::MAX
    }
}
