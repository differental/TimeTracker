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
    extract::{Query, Request},
    http::{StatusCode, header::AUTHORIZATION},
    middleware::Next,
    response::IntoResponse,
};
use serde::Deserialize;

use crate::constants::ACCESS_KEY;

#[derive(Deserialize)]
pub struct AuthQueryParams {
    key: Option<String>,
}

pub async fn auth_user(
    Query(params): Query<AuthQueryParams>,
    request: Request,
    next: Next,
) -> impl IntoResponse {
    let bearer = request
        .headers()
        .get(AUTHORIZATION)
        .and_then(|value| value.to_str().ok())
        .and_then(|value| value.strip_prefix("Bearer "))
        .map(str::trim);
    let query_key = params.key.as_deref().map(str::trim);

    // Keep query authentication for the existing web client, while native
    // clients use the Authorization header so credentials don't appear in URLs.
    if bearer == Some(ACCESS_KEY.as_str()) || query_key == Some(ACCESS_KEY.as_str()) {
        return next.run(request).await;
    }

    (StatusCode::FORBIDDEN).into_response()
}
