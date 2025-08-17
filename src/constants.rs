use sled::Tree;
use std::{env, sync::LazyLock};

pub static ACCESS_KEY: LazyLock<String> = LazyLock::new(|| env::var("ACCESS_KEY").unwrap());

#[derive(Clone)]
pub struct AppState {
    pub events: Tree,
    pub meta: Tree,
}

pub static STATES: [&str; 10] = [
    ("📚 Study"),
    ("💼 Work"),
    ("🚃 Commute"),
    ("🚣‍♂️ Sports"),
    ("📺 Entertainment"),
    ("📆 Appointment"),
    ("🥪 Maintenance"),
    ("🛏️ Sleep"),
    ("💬 Social"),
    ("🍹 Day Out"),
];
