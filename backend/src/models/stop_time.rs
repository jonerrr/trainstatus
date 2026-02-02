use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use sqlx::FromRow;
use utoipa::ToSchema;
use uuid::Uuid;

#[derive(PartialEq, Clone, Serialize, Deserialize, Hash, Eq, FromRow, ToSchema)]
pub struct StopTime {
    pub trip_id: Uuid,
    pub stop_id: String,
    pub arrival: DateTime<Utc>,
    pub departure: DateTime<Utc>,
    #[sqlx(json)]
    pub data: StopTimeData,
}

#[derive(PartialEq, Clone, Serialize, Deserialize, ToSchema, Hash, Eq)]
#[serde(tag = "source", rename_all = "snake_case")]
pub enum StopTimeData {
    MtaSubway {
        #[serde(skip_serializing_if = "Option::is_none")]
        #[schema(example = "B2")]
        scheduled_track: Option<String>,
        #[serde(skip_serializing_if = "Option::is_none")]
        #[schema(example = "B2")]
        actual_track: Option<String>,
    },
    MtaBus,
}
