use super::errors::ServerError;
use crate::{
    realtime::{alert::Alert, stop_time::StopTime, trip::Trip},
    AppState,
};
use axum::{
    extract::{Query, State},
    response::{
        sse::{Event, Sse},
        IntoResponse,
    },
};
use chrono::Utc;
use futures::stream::{self, Stream};
use serde::Deserialize;
use serde_json::json;
use std::time::Duration;
use tokio_stream::{wrappers::BroadcastStream, StreamExt as _};

#[derive(Deserialize)]
pub struct Parameters {
    // #[serde(deserialize_with = "parse_list", default = "all_stops")]
    // pub stop_ids: Vec<String>,
    // default is false
    // #[serde(default, rename = "static")]
    // static_data: bool,
    #[serde(default)]
    train: bool,
    #[serde(default)]
    bus: bool,
}

pub async fn sse_handler(
    State(state): State<AppState>,
    params: Query<Parameters>,
) -> Result<impl IntoResponse, ServerError> {
    if !params.train && !params.bus {
        return Err(ServerError::BadRequest);
    }

    let mut shutdown_rx = state.shutdown_tx.subscribe();
    let rx = state.tx.subscribe();

    // TODO: fetch at same time
    let current_trips = Trip::get_all(&state.pg_pool, Utc::now()).await?;
    let stop_times = StopTime::get_all(&state.pg_pool, Utc::now()).await?;
    let alerts = Alert::get_all(&state.pg_pool, Utc::now()).await?;

    let data = json!({
        "trips": current_trips,
        "alerts": alerts,
        "stop_times": stop_times,
    });

    let initial_event = Event::default()
        .json_data(data)
        .map_err(ServerError::Axum)?;
    let initial_stream = stream::once(async { Ok::<_, ServerError>(initial_event) });

    let stream = BroadcastStream::new(rx).map(|result| {
        result
            .map_err(ServerError::Broadcast)
            .and_then(|msg| Event::default().json_data(msg).map_err(ServerError::Axum))
    });

    let stream = initial_stream.chain(stream);

    let stream = stream::StreamExt::take_until(stream, async move {
        shutdown_rx.recv().await.expect("shutdown_rx");
    });

    Ok(Sse::new(stream)
        .keep_alive(
            axum::response::sse::KeepAlive::new().interval(Duration::from_secs(10)), // .text("keep-alive-text"),
        )
        .into_response())
}
