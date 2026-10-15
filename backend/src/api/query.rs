use chrono::{DateTime, Utc};
use serde::Deserialize;
use utoipa::IntoParams;

#[derive(Deserialize, IntoParams)]
#[into_params(parameter_in = Query)]
pub(super) struct TimeQuery {
    /// Unix timestamp used for a historical query. If omitted, the latest committed realtime snapshot is returned.
    #[serde(default, with = "chrono::serde::ts_seconds_option")]
    #[param(value_type = Option<i64>)]
    pub at: Option<DateTime<Utc>>,
}
