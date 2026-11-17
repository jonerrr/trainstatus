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
    #[serde(default, deserialize_with = "historical_time")]
    #[param(value_type = Option<i64>)]
    pub at: Option<DateTime<Utc>>,
}

fn historical_time<'de, D>(deserializer: D) -> Result<Option<DateTime<Utc>>, D::Error>
where
    D: serde::Deserializer<'de>,
{
    let at = chrono::serde::ts_seconds_option::deserialize(deserializer)?;
    if at.is_some_and(|at| {
        at.checked_add_signed(chrono::Duration::hours(4)).is_none()
            || at
                .checked_sub_signed(chrono::Duration::minutes(5))
                .is_none()
    }) {
        return Err(serde::de::Error::custom(
            "at cannot represent the complete historical time window",
        ));
    }
    Ok(at)
}

#[cfg(test)]
mod tests {
    use super::TimeQuery;
    use chrono::{DateTime, Utc};

    #[test]
    fn historical_time_requires_representable_visit_and_anchor_windows() {
        for at in [
            DateTime::<Utc>::MAX_UTC.timestamp(),
            DateTime::<Utc>::MIN_UTC.timestamp(),
        ] {
            assert!(serde_json::from_value::<TimeQuery>(serde_json::json!({ "at": at })).is_err());
        }
        let at = 1_794_518_580_i64;
        let parsed: TimeQuery = serde_json::from_value(serde_json::json!({ "at": at })).unwrap();
        assert_eq!(parsed.at.unwrap().timestamp(), at);
    }
}
