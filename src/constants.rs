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

pub static STATES_WITH_DESCRIPTIONS: [(&str, &str); 12] = [
    (
        "📚 Study",
        "Academic study, including course-related work and focused interview-preparation study.",
    ),
    (
        "💼 Work",
        "Internship or professional work tasks, whether performed in the office or remotely from home.",
    ),
    (
        "🚃 Commute",
        "Regular travel to and from a fixed destination such as office or class. Spontaneous travel, tourism, or trips taken for leisure are NOT counted here.",
    ),
    (
        "💻 Projects",
        "Work on independent, non-coursework projects — personal or group.",
    ),
    (
        "📺 Entertainment",
        "All forms of entertainment and leisure activities, indoors or outdoors.",
    ),
    (
        "💡 Exploration",
        "Casual and interest-driven learning and exploration. This generally includes watching explanatory YouTube videos, reading blog posts, or watching documentaries.",
    ),
    (
        "🥪 Maintenance",
        "Routine personal maintenance: purchasing, preparing, and consuming food or drinks, as well as quick personal breaks such as toilet breaks. Meals longer than one hour should only have their first hour counted towards \"Maintenance\".",
    ),
    (
        "🛏️ Sleep",
        "Time spent in bed for sleep or rest. Naps included.",
    ),
    (
        "👔 Mission",
        "Fulfilling personal responsibilities or duties to family, friends, or others.",
    ),
    (
        "📆 Appointment",
        "Scheduled appointments or meetings. This includes interviews, meetings, and career-related coffee-chats or meals. This does not include routine meetings at work.",
    ),
    (
        "💬 Social",
        "Time spent actively socialising with close friends or acquaintances.",
    ),
    (
        "🚣‍♂️ Sports",
        "Physical and sporting activities, including training and competition.",
    ),
];
