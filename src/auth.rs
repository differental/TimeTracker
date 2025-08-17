use axum::{
    extract::{Query, Request},
    http::StatusCode,
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
    // Authentication layer, checks query param against key

    if let Some(ref val) = params.key {
        if val.trim() != *ACCESS_KEY {
            return (StatusCode::FORBIDDEN).into_response();
        }
    } else {
        return (StatusCode::FORBIDDEN).into_response();
    }

    next.run(request).await
}
