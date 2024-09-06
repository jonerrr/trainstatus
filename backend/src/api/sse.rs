use super::errors::ServerError;
use crate::AppState;
use axum::{
    extract::State,
    response::sse::{Event, Sse},
};
use futures::stream::{self, Stream};
use std::time::Duration;
use tokio_stream::{wrappers::BroadcastStream, StreamExt as _};

pub async fn sse_handler(
    State(state): State<AppState>,
) -> Sse<impl Stream<Item = Result<Event, ServerError>>> {
    // A `Stream` that repeats an event every second
    //
    // You can also create streams from tokio channels using the wrappers in
    // https://docs.rs/tokio-stream
    // let stream = stream::repeat_with(|| Event::default().data("hi!"))
    //     .map(Ok)
    //     .throttle(Duration::from_secs(1));
    let mut shutdown_rx = state.shutdown_tx.subscribe();
    let rx = state.tx.subscribe();

    let stream = BroadcastStream::new(rx).map(|result| {
        result
            .map(|msg| {
                let data = serde_json::to_string(&msg).unwrap();
                Event::default().data(data)
            })
            .map_err(|_| ServerError::NotFound)
    });

    let stream = stream::StreamExt::take_until(stream, async move {
        shutdown_rx.recv().await.expect("shutdown_rx");
    });

    Sse::new(stream).keep_alive(
        axum::response::sse::KeepAlive::new().interval(Duration::from_secs(10)), // .text("keep-alive-text"),
    )
}
