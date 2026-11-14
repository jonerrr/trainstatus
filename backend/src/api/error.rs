use crate::operations::history::HistoryOverloaded;
use axum::response::{IntoResponse, Response};
use http::StatusCode;
use tracing::error;

pub struct AppError(anyhow::Error);

impl IntoResponse for AppError {
    fn into_response(self) -> Response {
        if self.0.downcast_ref::<HistoryOverloaded>().is_some() {
            return (
                StatusCode::SERVICE_UNAVAILABLE,
                "historical reads are overloaded; retry later",
            )
                .into_response();
        }
        error!(
            error.message = %format_args!("{:#}", self.0),
            "Internal server error"
        );

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
