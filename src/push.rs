use std::{
    env, fs,
    time::{SystemTime, UNIX_EPOCH},
};

use axum::{Json, extract::State, http::StatusCode};
use jsonwebtoken::{Algorithm, EncodingKey, Header, encode};
use reqwest::Client;
use serde::{Deserialize, Serialize};
use serde_json::{Value, json};

use crate::constants::AppState;

#[derive(Clone, Deserialize, Serialize)]
struct PushRegistration {
    token: String,
    topic: String,
    environment: PushEnvironment,
    activity_id: Option<String>,
}

#[derive(Clone, Copy, Deserialize, Serialize)]
#[serde(rename_all = "lowercase")]
enum PushEnvironment {
    Sandbox,
    Production,
}

#[derive(Deserialize)]
pub struct PushRegistrationRequest {
    token: String,
    topic: String,
    environment: String,
    activity_id: Option<String>,
}

#[derive(Serialize)]
struct ApnsClaims<'a> {
    iss: &'a str,
    iat: u64,
}

struct ApnsConfiguration {
    key_id: String,
    team_id: String,
    private_key: String,
}

pub async fn register_widget_push(
    State(state): State<AppState>,
    Json(request): Json<PushRegistrationRequest>,
) -> (StatusCode, Json<Value>) {
    register(state, request, RegistrationKind::Widget)
}

pub async fn register_live_activity_push(
    State(state): State<AppState>,
    Json(request): Json<PushRegistrationRequest>,
) -> (StatusCode, Json<Value>) {
    register(state, request, RegistrationKind::LiveActivity)
}

enum RegistrationKind {
    Widget,
    LiveActivity,
}

fn register(
    state: AppState,
    request: PushRegistrationRequest,
    kind: RegistrationKind,
) -> (StatusCode, Json<Value>) {
    let environment = match request.environment.as_str() {
        "sandbox" => PushEnvironment::Sandbox,
        "production" => PushEnvironment::Production,
        _ => {
            return (
                StatusCode::BAD_REQUEST,
                Json(json!({"error": "Invalid environment"})),
            );
        }
    };

    if request.token.len() < 32
        || !request.token.bytes().all(|byte| byte.is_ascii_hexdigit())
        || request.topic.len() > 255
        || request.topic.bytes().any(|byte| byte.is_ascii_whitespace())
    {
        return (
            StatusCode::BAD_REQUEST,
            Json(json!({"error": "Invalid push registration"})),
        );
    }

    let topic_prefix =
        env::var("APNS_TOPIC_PREFIX").unwrap_or_else(|_| "at.janez.TimeTracker".into());
    if !request.topic.starts_with(&topic_prefix) {
        return (
            StatusCode::BAD_REQUEST,
            Json(json!({"error": "Invalid APNs topic"})),
        );
    }

    let activity_id = match kind {
        RegistrationKind::Widget => None,
        RegistrationKind::LiveActivity => match request.activity_id {
            Some(id) if !id.is_empty() && id.len() <= 128 => Some(id),
            _ => {
                return (
                    StatusCode::BAD_REQUEST,
                    Json(json!({"error": "Missing Live Activity identifier"})),
                );
            }
        },
    };
    let key_prefix = match kind {
        RegistrationKind::Widget => "widget",
        RegistrationKind::LiveActivity => "live",
    };
    let key = format!("{key_prefix}:{}", request.token);
    let registration = PushRegistration {
        token: request.token,
        topic: request.topic,
        environment,
        activity_id,
    };

    match serde_json::to_vec(&registration)
        .ok()
        .and_then(|value| state.push_registrations.insert(key.as_bytes(), value).ok())
    {
        Some(_) => (StatusCode::OK, Json(json!({}))),
        None => (
            StatusCode::INTERNAL_SERVER_ERROR,
            Json(json!({"error": "Could not save push registration"})),
        ),
    }
}

pub fn notify_state_change(state: AppState, state_id: u8, start_timestamp: i64) {
    tokio::spawn(async move {
        if let Err(error) = send_state_change(&state, state_id, start_timestamp).await {
            eprintln!("APNs update failed: {error}");
        }
    });
}

async fn send_state_change(
    state: &AppState,
    state_id: u8,
    start_timestamp: i64,
) -> anyhow::Result<()> {
    let Some(config) = load_configuration()? else {
        return Ok(());
    };
    let client = Client::builder().http2_adaptive_window(true).build()?;
    let authorization = create_authorization(&config)?;
    let registrations = state
        .push_registrations
        .iter()
        .filter_map(Result::ok)
        .filter_map(|(key, value)| {
            serde_json::from_slice::<PushRegistration>(&value)
                .ok()
                .map(|registration| (key, registration))
        })
        .collect::<Vec<_>>();

    for (storage_key, registration) in registrations {
        let (push_type, priority, payload) = if registration.activity_id.is_some() {
            (
                "liveactivity",
                "10",
                json!({
                    "aps": {
                        "timestamp": unix_time(),
                        "event": "update",
                        "content-state": {
                            "stateID": state_id,
                            "startTimestamp": start_timestamp
                        },
                        "stale-date": (start_timestamp / 1_000) + (8 * 60 * 60)
                    }
                }),
            )
        } else {
            ("widgets", "5", json!({"aps": {"content-changed": true}}))
        };
        let host = match registration.environment {
            PushEnvironment::Sandbox => "https://api.sandbox.push.apple.com",
            PushEnvironment::Production => "https://api.push.apple.com",
        };
        let response = client
            .post(format!("{host}/3/device/{}", registration.token))
            .header("authorization", &authorization)
            .header("apns-push-type", push_type)
            .header("apns-topic", &registration.topic)
            .header("apns-priority", priority)
            .header("apns-expiration", "0")
            .json(&payload)
            .send()
            .await?;

        if response.status() == reqwest::StatusCode::GONE {
            state.push_registrations.remove(storage_key)?;
        } else if !response.status().is_success() {
            let status = response.status();
            let body = response.text().await.unwrap_or_default();
            eprintln!("APNs rejected {} ({status}): {body}", registration.topic);
        }
    }
    Ok(())
}

fn load_configuration() -> anyhow::Result<Option<ApnsConfiguration>> {
    let (Ok(key_path), Ok(key_id), Ok(team_id)) = (
        env::var("APNS_KEY_PATH"),
        env::var("APNS_KEY_ID"),
        env::var("APNS_TEAM_ID"),
    ) else {
        return Ok(None);
    };
    Ok(Some(ApnsConfiguration {
        key_id,
        team_id,
        private_key: fs::read_to_string(key_path)?,
    }))
}

fn create_authorization(config: &ApnsConfiguration) -> anyhow::Result<String> {
    let mut header = Header::new(Algorithm::ES256);
    header.kid = Some(config.key_id.clone());
    let claims = ApnsClaims {
        iss: &config.team_id,
        iat: unix_time(),
    };
    let token = encode(
        &header,
        &claims,
        &EncodingKey::from_ec_pem(config.private_key.as_bytes())?,
    )?;
    Ok(format!("bearer {token}"))
}

fn unix_time() -> u64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap_or_default()
        .as_secs()
}
