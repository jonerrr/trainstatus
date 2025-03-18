use crate::{static_data::cache_all, AppState};
use axum::{
    extract::{FromRequestParts, Query},
    response::{IntoResponse, Response},
};
use chrono::{DateTime, TimeZone, Utc};
use http::{request::Parts, HeaderMap};
use redis::AsyncCommands;
use serde::{Deserialize, Deserializer};
use std::sync::OnceLock;
use utoipa::IntoParams;
use utoipa_axum::{router::OpenApiRouter, routes};

pub mod errors;
pub mod realtime;
// pub mod websocket;
pub mod static_data;

pub fn router(state: AppState) -> OpenApiRouter {
    OpenApiRouter::new()
        .routes(routes!(realtime::alerts_handler))
        .routes(routes!(realtime::stop_times_handler))
        .routes(routes!(realtime::trips_handler))
        .routes(routes!(static_data::routes_handler))
        .routes(routes!(static_data::stops_handler))
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

impl AppState {
    // wrapper for redis get that handles when the cache is reset
    // only use for getting static data
    pub async fn get_from_cache(&self, key: &str) -> Result<String, errors::ServerError> {
        let mut conn = self.redis_pool.get().await?;
        let value: String = match conn.get(key).await {
            Ok(value) => value,
            Err(err) => {
                // if theres a type error, that means the cache probably got reset
                if err.kind() == redis::ErrorKind::TypeError {
                    // recache static data
                    cache_all(&self.pg_pool, &self.redis_pool).await?;

                    // conn.get(key).await?
                    return Box::pin(self.get_from_cache(key)).await;
                }
                return Err(errors::ServerError::Redis(err));
            }
        };
        Ok(value)
    }

    // same as above but for mget
    // currently only used for getting stop/route and hash
    pub async fn mget_from_cache(
        &self,
        keys: &[&str; 2],
    ) -> Result<(String, String), errors::ServerError> {
        let mut conn = self.redis_pool.get().await?;
        let values: (String, String) = match conn.mget(keys).await {
            Ok(values) => values,
            Err(err) => {
                // if theres a type error, that means the cache probably got reset
                if err.kind() == redis::ErrorKind::TypeError {
                    // recache static data
                    cache_all(&self.pg_pool, &self.redis_pool).await?;

                    // conn.mget(keys).await?
                    return Box::pin(self.mget_from_cache(keys)).await;
                }
                return Err(errors::ServerError::Redis(err));
            }
        };
        Ok(values)
    }
}

// this represents the current time to use for sql queries. default is current time, bool represents if user specified a time
// #[derive(Debug)]
pub struct CurrentTime {
    pub time: DateTime<Utc>,
    pub user_specified: bool,
    pub finished: bool,
}

#[derive(Deserialize, IntoParams)]
#[into_params(parameter_in = Query)]
pub struct TimeParams {
    /// Unix timestamp to use as the current time. If not specified, the current time is used.
    #[serde(default)]
    pub at: Option<i64>,
    /// Include trips / stop times that have finished. Mainly used for charts. DOES NOT AFFECT ALERTS CURRENTLY
    #[serde(default)]
    pub finished: bool,
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
                        finished: query.finished,
                    },
                    _ => {
                        // TODO: maybe return a 400 instead of logging
                        tracing::error!("Invalid timestamp: {}", at);
                        CurrentTime {
                            time: Utc::now(),
                            user_specified: false,
                            finished: query.finished,
                        }
                    }
                }
            }
            None => {
                let now = chrono::Utc::now();
                CurrentTime {
                    time: now,
                    user_specified: false,
                    finished: query.finished,
                }
            }
        };

        Ok(time)
    }
}
