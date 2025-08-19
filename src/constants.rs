use sled::Tree;
use std::{env, sync::LazyLock};

pub static ACCESS_KEY: LazyLock<String> = LazyLock::new(|| env::var("ACCESS_KEY").unwrap());

#[derive(Clone)]
pub struct AppState {
    pub events: Tree,
    pub meta: Tree,
}

pub static STATES: [&str; 12] = [
    ("📚 Study"),
    ("💼 Work"),
    ("🚃 Commute"),
    ("💻 Projects"),
    ("📺 Entertainment"),
    ("💡 Exploration"),
    ("🥪 Maintenance"),
    ("🛏️ Sleep"),
    ("👔 Mission"),
    ("📆 Appointment"),
    ("💬 Social"),
    ("🚣‍♂️ Sports"),
];
