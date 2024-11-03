use axum::{
    response::{IntoResponse, Response},
    Json,
};
use http::StatusCode;
use serde_json::json;
use tokio_stream::wrappers::errors::BroadcastStreamRecvError;

#[derive(thiserror::Error, Debug)]
#[error("...")]
pub enum ServerError {
    #[error("{0}")]
    Database(#[from] sqlx::Error),
    #[error("{0}")]
    Redis(#[from] redis::RedisError),
    #[error("{0}")]
    RedisPool(#[from] bb8::RunError<redis::RedisError>),
    #[error("{0}")]
    Axum(#[from] axum::Error),
    #[error("{0}")]
    Broadcast(#[from] BroadcastStreamRecvError),
    #[error("{0}")]
    SerdeJson(#[from] serde_json::Error),
    // #[error("Bad request")]
    // BadRequest,
    // #[error("Not found")]
    // NotFound,
}

impl IntoResponse for ServerError {
    fn into_response(self) -> Response {
        tracing::error!("{:#?}", self);

        let (status_code, message) = match self {
            ServerError::Database(_) => (StatusCode::INTERNAL_SERVER_ERROR, "database error"),
            ServerError::Redis(_) | ServerError::RedisPool(_) => {
                (StatusCode::INTERNAL_SERVER_ERROR, "cache error")
            }
            ServerError::SerdeJson(_) => (StatusCode::INTERNAL_SERVER_ERROR, "json error"),
            ServerError::Broadcast(_) => (StatusCode::INTERNAL_SERVER_ERROR, "broadcast error"),
            ServerError::Axum(_) => (StatusCode::INTERNAL_SERVER_ERROR, "stream error"),
            // ServerError::BadRequest => (StatusCode::BAD_REQUEST, "bad request"),
            // ServerError::NotFound => (StatusCode::NOT_FOUND, "not found"),
        };

        let body = Json(json!({ "message": message }));

        (status_code, body).into_response()
    }
}
