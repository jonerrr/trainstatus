use axum::extract::{FromRequestParts, Path};
use axum::http::request::Parts;
use chrono::{DateTime, Utc};
use serde::Deserialize;
use utoipa::IntoParams;

use crate::source::Source;

/// Validated `{source}` path segment: same extraction and rejection as [`Path<Source>`],
/// and records `trainstatus.source` on the active HTTP server span.
pub struct SourcePath(pub Source);

impl<S> FromRequestParts<S> for SourcePath
where
    S: Send + Sync,
{
    type Rejection = <Path<Source> as FromRequestParts<S>>::Rejection;

    async fn from_request_parts(parts: &mut Parts, state: &S) -> Result<Self, Self::Rejection> {
        let Path(source) = Path::<Source>::from_request_parts(parts, state).await?;
        tracing::Span::current().record("trainstatus.source", source.as_str());
        Ok(Self(source))
    }
}

#[derive(Deserialize, IntoParams)]
#[into_params(parameter_in = Query)]
pub struct TimeQuery {
    /// Unix timestamp used for a historical query. If omitted, the latest committed realtime snapshot is returned.
    #[serde(default, with = "chrono::serde::ts_seconds_option")]
    #[param(value_type = Option<i64>)]
    pub at: Option<DateTime<Utc>>,
}
