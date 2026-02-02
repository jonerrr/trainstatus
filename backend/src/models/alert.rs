use crate::{impl_discriminated_data, models::source::Source};
use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use uuid::Uuid;

/// Represents an alert with its translations stored separately
#[derive(Debug, Clone)]
pub struct Alert {
    pub id: Uuid,
    pub original_id: String,
    pub source: Source,
    pub created_at: DateTime<Utc>,
    pub updated_at: DateTime<Utc>,
    /// Used to prevent stale alerts
    pub recorded_at: DateTime<Utc>,
    pub data: AlertData,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, sqlx::Type)]
#[sqlx(type_name = "alert_section", rename_all = "snake_case")]
pub enum AlertSection {
    Header,
    Description,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, sqlx::Type)]
#[sqlx(type_name = "alert_format", rename_all = "snake_case")]
pub enum AlertFormat {
    Plain,
    Html,
}

/// Alert translations for different content types
#[derive(Debug, Clone)]
pub struct AlertTranslation {
    pub alert_id: Uuid,
    pub section: AlertSection,
    pub format: AlertFormat,
    pub language: String,
    pub text: String,
}

// used for mta subway and bus
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct MtaData {
    pub display_before_active: i32,
    pub alert_type: String,
    /// The id of the planned work this alert was cloned from
    pub clone_id: Option<String>,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(tag = "source", rename_all = "snake_case")]
pub enum AlertData {
    MtaSubway(MtaData),
    MtaBus(MtaData),
}

impl_discriminated_data!(
    AlertData,
    Source,
    {
        MtaBus => MtaData,
        MtaSubway => MtaData,
    }
);

#[derive(Debug, Clone)]
pub struct ActivePeriod {
    pub alert_id: Uuid,
    pub start_time: DateTime<Utc>,
    pub end_time: Option<DateTime<Utc>>,
}

#[derive(Debug, Clone)]
pub struct AffectedEntity {
    pub alert_id: Uuid,
    pub route_id: Option<String>,
    pub source: Source,
    pub stop_id: Option<String>,
    pub sort_order: i32,
}
