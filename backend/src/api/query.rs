//! Shared query parsing for live and historical requests.

use axum::{
    extract::{FromRequestParts, Query},
    response::{IntoResponse, Response},
};
use chrono::{DateTime, TimeZone, Utc};
use http::{StatusCode, request::Parts};
use serde::{Deserialize, Deserializer};
use utoipa::IntoParams;

#[derive(Deserialize, IntoParams)]
#[into_params(parameter_in = Query)]
pub(super) struct TimeQuery {
    /// Unix timestamp used for a historical query. If omitted, the latest committed realtime snapshot is returned.
    pub at: Option<i64>,
}

/// An omitted timestamp selects live data; a supplied timestamp selects history.
pub(super) struct RequestTime {
    pub at: Option<DateTime<Utc>>,
}

impl<S: Send + Sync> FromRequestParts<S> for RequestTime {
    type Rejection = Response;

    async fn from_request_parts(parts: &mut Parts, state: &S) -> Result<Self, Self::Rejection> {
        let Query(query) = Query::<TimeQuery>::from_request_parts(parts, state)
            .await
            .map_err(IntoResponse::into_response)?;
        Ok(Self {
            at: historical_time(query.at).map_err(IntoResponse::into_response)?,
        })
    }
}

pub(super) fn historical_time(
    at: Option<i64>,
) -> Result<Option<DateTime<Utc>>, (StatusCode, &'static str)> {
    at.map(|at| {
        Utc.timestamp_opt(at, 0).single().ok_or((
            StatusCode::BAD_REQUEST,
            "at must be a valid Unix timestamp in seconds",
        ))
    })
    .transpose()
}

pub(super) fn parse_list<'de, D>(deserializer: D) -> Result<Vec<String>, D::Error>
where
    D: Deserializer<'de>,
{
    let list = String::deserialize(deserializer)?;
    Ok(list.split(',').map(str::to_owned).collect())
}
