use axum::{
    response::{IntoResponse, Response},
    Json,
};
use http::StatusCode;
use serde_json::json;

#[derive(thiserror::Error, Debug)]
#[error("...")]
pub enum ServerError {
    #[error("{0}")]
    Database(#[from] sqlx::Error),
}

impl IntoResponse for ServerError {
    fn into_response(self) -> Response {
        tracing::error!("{:#?}", self.to_string());

        let (status_code, message) = match self {
            ServerError::Database(_) => (
                StatusCode::INTERNAL_SERVER_ERROR,
                "Database error".to_string(),
            ),
        };

        let body = Json(json!({ "message": message }));

        (status_code, body).into_response()
    }
}
