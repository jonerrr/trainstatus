use crate::AppState;
use axum::{
    extract::{FromRequestParts, Query},
    response::{IntoResponse, Response},
};
use chrono::{DateTime, TimeZone, Utc};
use http::{HeaderMap, StatusCode, request::Parts};
use serde::{Deserialize, Deserializer};
use std::sync::OnceLock;
use tracing::error;
use utoipa::IntoParams;
use utoipa_axum::{router::OpenApiRouter, routes};

// pub mod websocket;
pub mod health;
pub mod realtime;
pub mod static_data;
pub mod trajectory;
pub mod util;

pub struct AppError(anyhow::Error);

impl IntoResponse for AppError {
    fn into_response(self) -> Response {
        error!(error = %self.0, "Internal server error");

        (StatusCode::INTERNAL_SERVER_ERROR, "Something went wrong :(").into_response()
    }
}

// This enables using `?` on functions that return `Result<_, anyhow::Error>` to turn them into
// `Result<_, AppError>`. That way you don't need to do that manually.
impl<E> From<E> for AppError
where
    E: Into<anyhow::Error>,
{
    fn from(err: E) -> Self {
        Self(err.into())
    }
}

pub fn router(state: AppState) -> OpenApiRouter {
    OpenApiRouter::new()
        .route("/health", axum::routing::get(health::liveness_handler))
        .routes(routes!(health::liveness_handler))
        .routes(routes!(health::readiness_handler))
        .routes(routes!(static_data::routes_handler))
        .routes(routes!(static_data::stops_handler))
        .routes(routes!(realtime::trips_handler))
        .routes(routes!(realtime::stop_times_handler))
        .routes(routes!(realtime::positions_handler))
        .routes(routes!(realtime::alerts_handler))
        .routes(routes!(trajectory::trajectories_handler))
        .with_state(state)
}

// not sure if its better to do a oncelock headermap and clone or to just create headermap everytime
pub fn json_headers() -> &'static HeaderMap {
    static HEADERS: OnceLock<HeaderMap> = OnceLock::new();
    HEADERS.get_or_init(|| {
        let mut headers = HeaderMap::new();
        headers.insert("content-type", "application/json".parse().unwrap());
        headers
    })
}

pub fn parse_list<'de, D>(deserializer: D) -> Result<Vec<String>, D::Error>
where
    D: Deserializer<'de>,
{
    let str_sequence = String::deserialize(deserializer)?;

    Ok(str_sequence
        .split(',')
        .map(|item| item.to_owned())
        .collect())
}

pub struct CurrentTime {
    pub time: DateTime<Utc>,
    pub user_specified: bool,
    // not used currently
    // pub finished: bool,
}

#[derive(Deserialize, IntoParams)]
#[into_params(parameter_in = Query)]
pub struct TimeParams {
    /// Unix timestamp used for a historical query. If omitted, the latest committed realtime snapshot is returned.
    #[serde(default)]
    pub at: Option<i64>,
}

impl<S> FromRequestParts<S> for CurrentTime
where
    S: Send + Sync + Clone,
{
    type Rejection = Response;

    async fn from_request_parts(parts: &mut Parts, state: &S) -> Result<Self, Self::Rejection> {
        //  check if there is a querystring param "at"
        //  if there is, parse it as a datetime
        //  if there isn't, use the current time

        let query = Query::<TimeParams>::from_request_parts(parts, state)
            .await
            .map_err(|err| err.into_response())?;

        let time = match query.at {
            Some(at) => {
                let time = Utc.timestamp_opt(at, 0);
                match time {
                    chrono::LocalResult::Single(time) => CurrentTime {
                        time,
                        user_specified: true,
                    },
                    _ => {
                        // TODO: maybe return a 400 instead of logging
                        tracing::error!(timestamp = at, "Invalid timestamp");
                        CurrentTime {
                            time: Utc::now(),
                            user_specified: false,
                        }
                    }
                }
            }
            None => {
                let now = chrono::Utc::now();
                CurrentTime {
                    time: now,
                    user_specified: false,
                }
            }
        };

        Ok(time)
    }
}
