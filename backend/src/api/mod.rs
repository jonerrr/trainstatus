use axum::{
    async_trait,
    extract::{FromRequestParts, Query},
    response::{IntoResponse, Response},
};
use chrono::{DateTime, TimeZone, Utc};
use http::{request::Parts, HeaderMap};
use serde::{Deserialize, Deserializer};
use std::sync::OnceLock;

pub mod errors;
pub mod realtime;
pub mod websocket;
// pub mod sse;
pub mod static_data;

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

// this represents the current time to use for sql queries. default is current time, bool represents if user specified a time
// #[derive(Debug)]
pub struct CurrentTime {
    pub time: DateTime<Utc>,
    pub user_specified: bool,
}

#[derive(Deserialize, Debug)]
pub struct QueryParams {
    pub at: Option<i64>,
}

#[async_trait]
impl<S> FromRequestParts<S> for CurrentTime
where
    S: Send + Sync + Clone,
{
    type Rejection = Response;

    async fn from_request_parts(parts: &mut Parts, state: &S) -> Result<Self, Self::Rejection> {
        //  check if there is a querystring param "at"
        //  if there is, parse it as a datetime
        //  if there isn't, use the current time

        let query = Query::<QueryParams>::from_request_parts(parts, state)
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
                        tracing::error!("Invalid timestamp: {}", at);
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
