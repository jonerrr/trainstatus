use axum::{
    async_trait,
    extract::{FromRequestParts, Query},
    response::{IntoResponse, Response},
};
use chrono::{DateTime, TimeZone, Utc};
use http::request::Parts;
use serde::{Deserialize, Deserializer};

pub mod alerts;
pub mod bus;
pub mod errors;
pub mod stops;
pub mod trips;

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

// this represents the current time to use for sql queries. default is current time
pub struct CurrentTime(pub DateTime<Utc>);

#[derive(Deserialize)]
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
                    chrono::LocalResult::Single(time) => CurrentTime(time),
                    _ => {
                        tracing::error!("Invalid timestamp: {}", at);
                        CurrentTime(Utc::now())
                    }
                }
            }
            None => {
                let now = chrono::Utc::now();
                CurrentTime(now)
            }
        };

        Ok(time)
    }
}
