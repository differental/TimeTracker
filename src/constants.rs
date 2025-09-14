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

use sled::Tree;
use std::{env, sync::LazyLock};

pub static ACCESS_KEY: LazyLock<String> = LazyLock::new(|| env::var("ACCESS_KEY").unwrap());

#[derive(Clone)]
pub struct AppState {
    pub events: Tree,
    pub meta: Tree,
}

pub static STATE_COUNT: usize = 15;

pub static EMERGENCY_STATE_INDEX: usize = 14;

#[derive(Clone, Copy)]
pub struct StateDetail<'a> {
    pub emoji: &'a str,
    pub name: &'a str,
    pub description: &'a str,
    pub colour: &'a str,
}

pub static ALL_STATES_DETAILS: [StateDetail; STATE_COUNT] = [
    StateDetail {
        emoji: "📚",
        name: "Study",
        description: "Academic study, including course-related work and focused interview-preparation study.",
        colour: "#4a71ea",
    },
    StateDetail {
        emoji: "💼",
        name: "Work",
        description: "Internship or professional work tasks, whether performed in the office or remotely from home.",
        colour: "#d4b37f",
    },
    StateDetail {
        emoji: "🚃",
        name: "Commute",
        description: "Regular travel to and from a fixed destination such as office or class. Spontaneous travel, tourism, or trips taken for leisure are NOT counted here.",
        colour: "#ff8c00",
    },
    StateDetail {
        emoji: "💻",
        name: "Projects",
        description: "Work on independent, non-coursework projects — personal or group.",
        colour: "#c49aff",
    },
    StateDetail {
        emoji: "📺",
        name: "Entertainment",
        description: "All forms of entertainment and leisure activities, indoors or outdoors.",
        colour: "#ffe066",
    },
    StateDetail {
        emoji: "💡",
        name: "Exploration",
        description: "Casual and interest-driven learning and exploration. This generally includes watching explanatory YouTube videos, reading blog posts, or watching documentaries.",
        colour: "#2ecc71",
    },
    StateDetail {
        emoji: "🥪",
        name: "Maintenance",
        description: "Routine personal maintenance: purchasing, preparing, and consuming food or drinks, as well as quick personal breaks such as toilet breaks. Meals longer than one hour should only have their first hour counted towards \"Maintenance\".",
        colour: "#b56a3b",
    },
    StateDetail {
        emoji: "🛏️",
        name: "Sleep",
        description: "Time spent in bed for sleep or rest. Naps included.",
        colour: "#ffd6e8",
    },
    StateDetail {
        emoji: "👔",
        name: "Mission",
        description: "Fulfilling personal responsibilities or duties to family, friends, or others.",
        colour: "#008080",
    },
    StateDetail {
        emoji: "📆",
        name: "Appointment",
        description: "Scheduled appointments or meetings. This includes interviews, meetings, and career-related coffee-chats or meals. This does not include routine meetings at work.",
        colour: "#6f42c1",
    },
    StateDetail {
        emoji: "💬",
        name: "Social",
        description: "Time spent actively socialising with close friends or acquaintances.",
        colour: "#ff6b6b",
    },
    StateDetail {
        emoji: "🚣‍♂️",
        name: "Sports",
        description: "Physical and sporting activities, including training and competition.",
        colour: "#e74c3c",
    },
    StateDetail {
        emoji: "🌴",
        name: "Holiday",
        description: "Spontaneous trips, alone or with friends & family. Travel to/from events also fall under this category, but study/work done during a \"holiday\" should be classed separately. If a meal forms a significant part of the experience, it should fall under this category, and Maintenance otherwise.",
        colour: "#fff9ba",
    },
    StateDetail {
        emoji: "⚫",
        name: "Other",
        description: "Items that don't fall under any category, or a temporary special marker for a certain event.",
        colour: "#000000",
    },
    StateDetail {
        emoji: "🚨",
        name: "Emergency",
        description: "Any emergencies that interrupt normal schedules. This should be undeclared as soon as the incident is no longer fully disrupting other scheduled activities.",
        colour: "#ff0000",
    },
];

pub static IDLE_STATE: StateDetail = StateDetail {
    emoji: "⏱️",
    name: "IDLE — Not recorded",
    description: "",
    colour: "#FFF",
};
